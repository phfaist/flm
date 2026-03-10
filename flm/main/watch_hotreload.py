import logging
logger = logging.getLogger(__name__)

import json
import threading


import asyncio
import websockets.asyncio.server as websockets_server
# import websockets.exceptions as websockets_exceptions


from html.parser import HTMLParser
from difflib import SequenceMatcher
from lxml import html

from .watch_util import find_available_port




def hotreload_js_code_to_inject(wsHost, wsPort):
    return f"""
window.addEventListener("DOMContentLoaded", function() {{
    var websocket = new WebSocket("ws://{wsHost}:{wsPort}/");
    websocket.addEventListener("message", function (m) {{
        console.log("Message!", m);
        var info = JSON.parse(m.data);
        if (info.action == 'update-content') {{
            var bodyElement = document.getElementById('Main');
            if (bodyElement == null || info.content_html == null) {{
                // either no "Main" element or the server couldn't extract
                // the new body content ... need a full reload.
                window.location.reload();
                return;
            }} else {{
                // replace "Main" content:
                bodyElement.innerHTML = info.content_html;
                // see if there is any further setup to do (MathJaX,
                // build toc, etc.) -- depends on the template
                if (window.flmSetup) {{
                     window.flmSetup();
                }}
            }}
        }}
    }});
    websocket.addEventListener("open", function () {{ console.log("websocket open"); }});
    websocket.addEventListener("close", function () {{ console.log("websocket closed"); }});
    websocket.addEventListener("error", function (err) {{
        console.log("websocket error", err);
    }} );
    console.log("Started websocket and listening for update messages.");
}});
"""


template_hr_begin_tag = '<!-- FLM_HOT_RELOAD_BEGIN_CONTENT -->'
template_hr_end_tag = '<!-- FLM_HOT_RELOAD_END_CONTENT -->'






class HotReloaderServer:

    def __init__(self, ws_host, ws_port):

        self.ws_host = ws_host

        if ws_port is not None:
            self.ws_port = ws_port
        else:
            self.ws_port = find_available_port(self.ws_host, 28102)


        self.ws_notify_update_queue = asyncio.Queue()

        self.ws_server_thread = None
        self.ws_loop = None

    def get_host(self):
        return self.ws_host
    def get_port(self):
        return self.ws_port

    def send_update_info(self, update_info):
        if self.ws_loop is None:
            raise RuntimeError("WS server/thread not set up yet.")

        #logger.debug("Queuing new ws update for web clients.")
        asyncio.run_coroutine_threadsafe(
            self.ws_notify_update_queue.put(json.dumps(update_info)),
            self.ws_loop,
        )


    def start_server(self):

        ws_connected_clients = []
        self.ws_loop = None

        async def ws_notify_update_clients():
            while True:
                # Wait for a message on the Queue
                message = await self.ws_notify_update_queue.get()
                if ws_connected_clients:
                    # Send to all connected clients
                    await asyncio.gather(*[
                        client.send(message)
                        for client in ws_connected_clients
                    ])
                    logger.info(f"Sent websocket update to {len(ws_connected_clients)} clients")

        async def ws_handler(client, path=None):
            # Handles new WebSocket connections.
            # Register the new websocket client.
            logger.info("Websocket client connected")
            ws_connected_clients.append(client)
            try:
                await client.wait_closed() # keep the connection open
            finally:
                ws_connected_clients.remove(client)

        async def ws_async_start_and_serve():
            # start the server and the message sender
            ws_server = await websockets_server.serve(ws_handler, self.ws_host, self.ws_port)
            logger.info(f"Websocket server started for automatic hot-reload")
            # on {ws_host}:{ws_port}

            await asyncio.gather(*[
                ws_server.wait_closed(), ws_notify_update_clients(),
            ])

        def ws_start_server_in_thread():
            self.ws_loop = asyncio.new_event_loop()
            asyncio.set_event_loop(self.ws_loop)
            self.ws_loop.run_until_complete( ws_async_start_and_serve() )

        ws_server_thread = threading.Thread(target=ws_start_server_in_thread, daemon=True)
        ws_server_thread.start()

        self.ws_server_thread = ws_server_thread






def make_hotreloader(computed_format, output, hotreload_server=None):
    if computed_format is not 'html':
        return HotReloaderDisabled()

    if hotreload_server is None:
        hotreload_server = HotReloaderServer('localhost', None)
        hotreload_server.start_server()

    return HotReloaderHtml(hotreload_server, computed_format, output)


class HotReloaderDisabled:
    def __init__(self, **kwargs):
        super().__init__()
        pass

    def is_enabled(self):
        return False

    def inject_hotreload_js(self):
        pass

    def new_run_send_update(self, info, content):
        pass



class HotReloaderHtml:
    def __init__(self, hotreload_server, computed_format, output):
        super().__init__()
        self.hotreload_server = hotreload_server
        self.computed_format = computed_format
        self.output = output

    def is_enabled(self):
        return True
    
    def inject_hotreload_js(self):
        fname = self.output

        with open(fname, 'r', encoding='utf-8') as f:
            content = f.read()

        # inject hot-reload code in generated HTML, if necessary
        ibodyend = content.rfind('</body>')
        if ibodyend == -1:
            logger.warning("Couldn't inject hot-reload JS code, didn't find </body>")
            return

        hotreload_js = (
            """<script type="text/javascript">"""
            + hotreload_js_code_to_inject(self.hotreload_server.get_host(),
                                          self.hotreload_server.get_port())
            + """</script>"""
        )

        content = (
            content[:ibodyend]
            + hotreload_js
            + content[ibodyend:]
        )

        with open(fname, 'w', encoding='utf-8') as fw:
            fw.write(content)


    def new_run_send_update(self, info, content):

        ibegin = content.find(template_hr_begin_tag)
        iend = content.rfind(template_hr_end_tag)
        if ibegin == -1 or iend == -1:
            logger.debug("Content hot-reloading not supported by this template")
            content_html = None
        else:
            content_html = content[ ibegin+len(template_hr_begin_tag) : iend ]

        # update hot-reload clients
        update_info = {
            "action": "update-content",
            "content_html": content_html,
        }

        self.hotreload_server.send_update_info(update_info)



#
# TODO, INCORPORATE LATER:
#



def diff_html(html_old, html_new):
    """Return a list of elements that contain any changes, with their new HTML.

    Each element is the most granular unambiguously locatable container
    (has an id, or is the root).  If one reported element is an ancestor
    of another, only the ancestor is reported.
    """
    old_changed, new_changed = _diff_offsets(html_old, html_new)
    if not old_changed and not new_changed:
        return []

    old_tree, old_intervals = _parse_with_offsets(html_old)
    new_tree, new_intervals = _parse_with_offsets(html_new)

    changed_elements = set()

    for off in new_changed:
        el = _deepest_element_at(new_intervals, off)
        if el is not None:
            changed_elements.add(el)

    for off in old_changed:
        el_old = _deepest_element_at(old_intervals, off)
        if el_old is not None:
            el_new = _find_corresponding(el_old, new_tree)
            if el_new is not None:
                changed_elements.add(el_new)

    reported = {_nearest_id_ancestor(el) for el in changed_elements}
    reported = _remove_descendants(reported)

    return [
        {
            'id': el.get('id'),
            'tag': el.tag,
            'html': html.tostring(el, encoding='unicode'),
        }
        for el in reported
    ]


# ---------------------------------------------------------------------------
# Text diff → changed offsets
# ---------------------------------------------------------------------------

def _diff_offsets(html_old, html_new):
    sm = SequenceMatcher(None, html_old, html_new, autojunk=False)
    old_changed = set()
    new_changed = set()
    for op, i1, i2, j1, j2 in sm.get_opcodes():
        if op == 'equal':
            continue
        old_changed.update(range(i1, max(i1 + 1, i2)))
        new_changed.update(range(j1, max(j1 + 1, j2)))
        if op == 'insert':
            old_changed.add(min(i1, len(html_old) - 1))
        elif op == 'delete':
            new_changed.add(min(j1, len(html_new) - 1))
    return old_changed, new_changed


# ---------------------------------------------------------------------------
# HTMLParser-based offset mapping
# ---------------------------------------------------------------------------

_VOID_TAGS = frozenset(
    'area base br col embed hr img input link meta param source track wbr'.split()
)


class _OffsetParser(HTMLParser):
    """Parse HTML and record (start_offset, end_offset) for each element."""

    def __init__(self, raw):
        super().__init__()
        self.raw = raw
        self.line_offsets = self._build_line_offsets(raw)
        self.stack = []
        self.intervals = []

    @staticmethod
    def _build_line_offsets(text):
        offsets = [0]
        for i, ch in enumerate(text):
            if ch == '\n':
                offsets.append(i + 1)
        return offsets

    def _pos_to_offset(self):
        line, col = self.getpos()
        return self.line_offsets[line - 1] + col

    def handle_starttag(self, tag, attrs):
        end_offset = self._pos_to_offset()
        start_text = self.get_starttag_text()
        start = end_offset - len(start_text) if start_text else end_offset
        if tag in _VOID_TAGS:
            self.intervals.append((start, end_offset, tag, dict(attrs)))
        else:
            self.stack.append((tag, start, dict(attrs)))

    def handle_endtag(self, tag):
        end_offset = self._pos_to_offset()
        for i in range(len(self.stack) - 1, -1, -1):
            if self.stack[i][0] == tag:
                open_tag, start, attrs = self.stack.pop(i)
                self.intervals.append((start, end_offset, tag, attrs))
                break

    def handle_startendtag(self, tag, attrs):
        end_offset = self._pos_to_offset()
        start_text = self.get_starttag_text()
        start = end_offset - len(start_text) if start_text else end_offset
        self.intervals.append((start, end_offset, tag, dict(attrs)))

    def close(self):
        super().close()
        for tag, start, attrs in self.stack:
            self.intervals.append((start, len(self.raw), tag, attrs))
        self.stack.clear()


def _parse_with_offsets(raw_html):
    """Return (lxml_tree, list of (start, end, lxml_element))."""
    tree = html.fragment_fromstring(raw_html, create_parent='div')

    parser = _OffsetParser(raw_html)
    parser.feed(raw_html)
    parser.close()

    # Sort: by start asc, then by size desc (parents before children)
    parser.intervals.sort(key=lambda x: (x[0], -(x[1] - x[0])))

    # Correlate parser intervals with lxml elements by greedy tag matching
    # in document order.
    lxml_els = [el for el in tree.iter() if isinstance(el.tag, str)]
    if lxml_els and lxml_els[0] is tree:
        lxml_els = lxml_els[1:]

    lxml_by_tag = {}
    for el in lxml_els:
        lxml_by_tag.setdefault(el.tag, []).append(el)

    tag_cursors = {tag: 0 for tag in lxml_by_tag}
    result = []
    for start, end, tag, attrs in parser.intervals:
        if tag in lxml_by_tag and tag_cursors.get(tag, 0) < len(lxml_by_tag[tag]):
            el = lxml_by_tag[tag][tag_cursors[tag]]
            tag_cursors[tag] += 1
            result.append((start, end, el))

    return tree, result


# ---------------------------------------------------------------------------
# Lookup helpers
# ---------------------------------------------------------------------------

def _deepest_element_at(intervals, offset):
    """Find the smallest element spanning the given offset."""
    best = None
    best_size = float('inf')
    for start, end, el in intervals:
        if start <= offset <= end:
            size = end - start
            if size < best_size:
                best_size = size
                best = el
    return best


def _find_corresponding(el_old, new_tree):
    """Find element in new_tree corresponding to el_old, by id.

    Walks up ancestors to find the nearest id match.  Falls back to root.
    """
    cur = el_old
    while cur is not None:
        el_id = cur.get('id')
        if el_id:
            found = new_tree.xpath(f'.//*[@id="{el_id}"]')
            if found:
                return found[0]
        cur = cur.getparent()
    return new_tree


def _nearest_id_ancestor(el):
    """Walk up to nearest element (including self) with an id, or root."""
    cur = el
    while cur is not None:
        if cur.get('id'):
            return cur
        parent = cur.getparent()
        if parent is None:
            return cur
        cur = parent
    return el


def _remove_descendants(elements):
    """Keep only elements that are not descendants of other elements in the set."""
    result = set()
    for el in elements:
        ancestor = el.getparent()
        is_descendant = False
        while ancestor is not None:
            if ancestor in elements:
                is_descendant = True
                break
            ancestor = ancestor.getparent()
        if not is_descendant:
            result.add(el)
    return result
