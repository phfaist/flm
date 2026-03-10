import logging
logger = logging.getLogger(__name__)

import os.path
import json
import threading


import asyncio
import websockets.asyncio.server as websockets_server
# import websockets.exceptions as websockets_exceptions


from lxml import html

from .watch_util import find_available_port




def hotreload_js_code_to_inject(wsHost, wsPort):
    js_src = os.path.realpath(os.path.join(os.path.dirname(__file__), 'watch_hotreload_inject.js'));
    with open(js_src, 'r', encoding='utf-8') as f:
        js_code = f.read()

    return f"""(function(window, wsHost, wsPort){{
{js_code}
}})(window,{json.dumps(wsHost)},{json.dumps(wsPort)});"""


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

        self.previous_content = None
        with open(self.output, 'r', encoding='utf-8') as f:
            self.previous_content = f.read()


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

        updates = diff_html(self.previous_content, content)

        # update hot-reload clients
        update_info = {
            "updates": updates,
            "content_html": content_html,
        }

        self.hotreload_server.send_update_info(update_info)

        # save this as the current content
        self.previous_content = content





def diff_html(html_old, html_new):
    """Return a list of granular structural updates from html_old to html_new.

    Each update is a dict with an 'action' key:

    'update-element-contents'
        An element's content (and/or attributes) changed in place.
        Keys: 'id' (str|None), 'html' (str).
        id=None → root container; html is the new full innerHTML.
        id=str  → outerHTML of the element with that id.

    'insert-after-element'
        A new element was inserted.
        Keys: 'after_ref' (ref|None), 'parent_id' (str|None), 'html' (str).
        after_ref=None  → insert before all siblings in parent.
        parent_id=None  → root container is the parent.

    'delete-element'
        An element was removed.
        Keys: 'ref' (ref).

    A *ref* is a list [anchor_id, sibling_offset, child_offset]:
        anchor_id      – id of the anchor element
        sibling_offset – how many siblings after the anchor (0 = anchor itself)
        child_offset   – index among anchor-sibling's children, or None if the
                         anchor-sibling is the target element itself
    """
    if html_old == html_new:
        return []

    old_root = html.fragment_fromstring(html_old, create_parent='div')
    new_root = html.fragment_fromstring(html_new, create_parent='div')

    updates = _diff_children(old_root, new_root)
    return _deduplicate_updates(updates)


# ---------------------------------------------------------------------------
# Tree diffing
# ---------------------------------------------------------------------------

def _diff_children(old_parent, new_parent):
    """Recursively diff element children, returning a flat list of updates."""
    old_kids = [c for c in old_parent if isinstance(c.tag, str)]
    new_kids = [c for c in new_parent if isinstance(c.tag, str)]

    matched, old_only, new_only = _match_children(old_kids, new_kids)

    updates = []
    parent_id = new_parent.get('id')  # None for synthetic root

    # Deletions
    for i in sorted(old_only):
        el = old_kids[i]
        ref = _element_ref(el, old_kids, old_parent)
        if ref is not None:
            updates.append({'action': 'delete-element', 'ref': ref})
        else:
            updates.append(_container_update(new_parent, parent_id))

    # Insertions
    for j in sorted(new_only):
        el = new_kids[j]
        updates.append({
            'action': 'insert-after-element',
            'after_ref': _preceding_ref(j, new_kids, new_parent),
            'html': html.tostring(el, encoding='unicode'),
        })

    # Matched pairs — recurse for finer-grained updates
    for i, j in matched:
        old_el = old_kids[i]
        new_el = new_kids[j]
        if html.tostring(old_el) == html.tostring(new_el):
            continue

        attrs_changed = dict(old_el.attrib) != dict(new_el.attrib)

        # Recurse only when attributes are unchanged (attribute changes are
        # not "inside" the element's body, so they can't be delegated further).
        if not attrs_changed:
            inner = _diff_children(old_el, new_el)
            if inner:
                updates.extend(inner)
                continue

        # Report at this level: use a positional ref for the element itself
        el_ref = _element_ref(new_el, new_kids, new_parent)
        if el_ref is not None:
            updates.append({
                'action': 'update-element-contents',
                'ref': el_ref,
                'html': html.tostring(new_el, encoding='unicode'),
            })
        else:
            updates.append(_container_update(new_parent, parent_id))

    return updates


def _container_update(new_parent, parent_id):
    """Build an update-element-contents for a parent/container element."""
    if parent_id is not None:
        return {
            'action': 'update-element-contents',
            'ref': [parent_id, 0, None],
            'html': html.tostring(new_parent, encoding='unicode'),
        }
    return {
        'action': 'update-element-contents',
        'ref': None,
        'html': _inner_html(new_parent),
    }


def _inner_html(el):
    """Return the innerHTML of el (text + serialized children including tails)."""
    parts = [el.text or '']
    for child in el:
        parts.append(html.tostring(child, encoding='unicode'))
    return ''.join(parts)


# ---------------------------------------------------------------------------
# Child matching
# ---------------------------------------------------------------------------

def _match_children(old_kids, new_kids):
    """Match old and new element children.

    Returns (matched, old_only, new_only):
      matched  – list of (old_idx, new_idx), order-crossing pairs removed
      old_only – set of old indices without a match
      new_only – set of new indices without a match
    """
    old_used = set()
    new_used = set()
    pairs = []

    # Phase 1: match by id
    new_id_idx = {c.get('id'): j for j, c in enumerate(new_kids) if c.get('id')}
    for i, c in enumerate(old_kids):
        el_id = c.get('id')
        if el_id and el_id in new_id_idx:
            j = new_id_idx[el_id]
            pairs.append((i, j))
            old_used.add(i)
            new_used.add(j)

    # Phase 2: match remaining by tag (greedy left-to-right)
    for i, c in enumerate(old_kids):
        if i in old_used:
            continue
        for j, nc in enumerate(new_kids):
            if j in new_used:
                continue
            if c.tag == nc.tag:
                pairs.append((i, j))
                old_used.add(i)
                new_used.add(j)
                break

    # Sort by new index, then drop crossing pairs (keep longest increasing
    # subsequence of old indices so neither side has order crossings).
    pairs.sort(key=lambda p: p[1])
    matched = [pairs[k] for k in _lis([i for i, _ in pairs])]

    old_only = set(range(len(old_kids))) - {i for i, _ in matched}
    new_only = set(range(len(new_kids))) - {j for _, j in matched}
    return matched, old_only, new_only


def _lis(seq):
    """Return indices into seq forming the longest strictly increasing subsequence."""
    n = len(seq)
    if n == 0:
        return []
    dp = [1] * n
    prev = [-1] * n
    for i in range(1, n):
        for j in range(i):
            if seq[j] < seq[i] and dp[j] + 1 > dp[i]:
                dp[i] = dp[j] + 1
                prev[i] = j
    best = max(range(n), key=lambda k: dp[k])
    path = []
    k = best
    while k >= 0:
        path.append(k)
        k = prev[k]
    return path[::-1]


# ---------------------------------------------------------------------------
# Element references
# ---------------------------------------------------------------------------

def _has_text_between(parent, start_node_or_none, end_node):
    """Return True if there is non-whitespace text between start_node_or_none
    and end_node in parent's raw child list (which includes comment nodes).

    sibling_offset and child_offset both count HTML *elements* only; a client
    skips whitespace text and comments when resolving them.  But if there is
    non-whitespace text content anywhere between the reference point and the
    target, the ref would be ambiguous, so callers should fall back.

    If start_node_or_none is None, the check starts from the beginning of
    parent (including parent.text before the first child).
    """
    all_children = list(parent)
    end_pos = all_children.index(end_node)

    if start_node_or_none is None:
        if parent.text and parent.text.strip():
            return True
        nodes_to_check = all_children[:end_pos]
    else:
        start_pos = all_children.index(start_node_or_none)
        nodes_to_check = all_children[start_pos:end_pos]

    for node in nodes_to_check:
        if node.tail and node.tail.strip():
            return True
    return False


def _element_ref(el, siblings, parent):
    """Return a [anchor_id, sibling_offset, child_offset] ref for el.

    sibling_offset and child_offset count HTML elements only (comments and
    whitespace-only text are skipped by the client).  Refs are only emitted
    when all content between the reference point and the target is free of
    non-whitespace text, so the element-counting is unambiguous.

    Tries, in order:
      1. Nearest id'd sibling at or before el: [anchor_id, offset, None]
         offset = number of element siblings between anchor and el.
      2. Parent's id with el's child index:    [parent_id, 0, child_idx]
         child_idx = el's position among element children of parent.
    Returns None if no reliable ref can be formed (caller should fall back).
    """
    idx = siblings.index(el)
    for k in range(idx, -1, -1):
        anchor_id = siblings[k].get('id')
        if anchor_id:
            if not _has_text_between(parent, siblings[k], el):
                return [anchor_id, idx - k, None]
    parent_id = parent.get('id') if parent is not None else None
    if parent_id:
        if not _has_text_between(parent, None, el):
            return [parent_id, 0, idx]
    return None


def _preceding_ref(j, kids, parent):
    """Ref for the element immediately before position j (for insert-after).

    Returns None when j==0 (insert at beginning of parent).
    """
    if j == 0:
        return None
    return _element_ref(kids[j - 1], kids, parent)


# ---------------------------------------------------------------------------
# Deduplication
# ---------------------------------------------------------------------------

def _deduplicate_updates(updates):
    """Remove duplicate updates targeting the same element."""
    seen_update_refs = set()
    seen_delete_refs = set()
    result = []
    for u in updates:
        action = u['action']
        if action == 'update-element-contents':
            key = tuple(u['ref']) if u['ref'] is not None else None
            if key not in seen_update_refs:
                seen_update_refs.add(key)
                result.append(u)
        elif action == 'delete-element':
            key = tuple(u['ref']) if u['ref'] is not None else None
            if key not in seen_delete_refs:
                seen_delete_refs.add(key)
                result.append(u)
        else:
            result.append(u)
    return result
