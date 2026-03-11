import logging
logger = logging.getLogger(__name__)

import re
import os.path
import json
import threading

from html import escape as html_escape

import asyncio
import websockets.asyncio.server as websockets_server
# import websockets.exceptions as websockets_exceptions

from .watch_util import find_available_port




def hotreload_js_code_to_inject(wsHost, wsPort):
    js_src = os.path.realpath(os.path.join(os.path.dirname(__file__), 'dist', 'watch_hotreload_inject.js'));
    with open(js_src, 'r', encoding='utf-8') as f:
        js_code = f.read()

    js_code = re.sub(r'^\/\/\# sourceMappingURL\=.*$', '', js_code, flags=re.MULTILINE)

    return f"""(function(window, wsHost, wsPort){{
{js_code}
}})(window,{json.dumps(wsHost)},{json.dumps(wsPort)});
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

        # Handlers for messages received from clients.
        # Maps action string -> callable(message_dict).
        self._client_message_handlers = {}

    def get_host(self):
        return self.ws_host
    def get_port(self):
        return self.ws_port

    def register_client_message_handler(self, action, handler):
        """Register a handler for client-initiated messages with the given action.

        The handler is called as ``handler(message_dict)`` in the main
        (non-async) thread via ``call_soon_threadsafe`` so it may safely
        interact with regular Python state.  For async work you should
        schedule it yourself.
        """
        self._client_message_handlers[action] = handler

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
                async for raw_message in client:
                    try:
                        msg = json.loads(raw_message)
                    except json.JSONDecodeError:
                        logger.warning(f"Received invalid JSON from websocket client: {raw_message!r}")
                        continue
                    action = msg.get('action')
                    handler = self._client_message_handlers.get(action)
                    if handler is not None:
                        try:
                            handler(msg)
                        except Exception:
                            logger.exception(f"Error in client message handler for action {action!r}")
                    else:
                        logger.warning(f"No handler registered for client message action {action!r}")
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




def _detect_editor(prefer_editor=None):
    """
    Return (name, callable) for the best available editor, or None.

    The callable signature is open_fn(file, line, col) where line and col
    may be None.  Each entry is tried in order; the first whose executable
    is found on PATH wins.
    """
    import shutil
    import subprocess

    def _run(args):
        proc = subprocess.Popen(args, start_new_session=True,
                                stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        def _check():
            try:
                _, stderr_data = proc.communicate(timeout=5)
                if proc.returncode != 0:
                    msg = stderr_data.decode(errors='replace').strip()
                    logger.warning(
                        f"Editor command failed (exit {proc.returncode}): {args}"
                        + (f"\n{msg}" if msg else "")
                    )
            except subprocess.TimeoutExpired:
                pass  # still running after 5 s — normal for GUI editors that fork
        threading.Thread(target=_check, daemon=True).start()

    def _vscode(file, line, col):
        loc = file
        if line is not None:
            loc = f"{file}:{line}"
            if col is not None:
                loc = f"{file}:{line}:{col}"
        _run(['code', '--goto', loc])

    def _emacs(file, line, col):
        # emacsclient supports +LINE:COL syntax.
        if line is not None:
            loc = f"+{line}:{col}" if col is not None else f"+{line}"
        else:
            loc = None
        args = ['emacsclient', '-n',] # do not use -c, want to use existing frame.
        if loc:
            args.append(loc)
        args.append(file)
        _run(args)

    def _gvim(file, line, col):
        args = ['gvim']
        if line is not None:
            args.append(f'+{line}')
        if col is not None:
            args += ['-c', f'normal! {col}|']
        args.append(file)
        _run(args)

    def _vim(file, line, col):
        args = ['vim']
        if line is not None:
            args.append(f'+{line}')
        if col is not None:
            args += ['-c', f'normal! {col}|']
        args.append(file)
        _run(args)

    def _sublime(file, line, col):
        loc = file
        if line is not None:
            loc = f"{file}:{line}"
            if col is not None:
                loc = f"{file}:{line}:{col}"
        _run(['subl', loc])

    candidates = [
        ('code',        'vscode',  _vscode),
        ('emacsclient', 'emacs',   _emacs),
        ('subl',        'sublime', _sublime),
        ('gvim',        'gvim',    _gvim),
        ('vim',         'vim',     _vim),
    ]

    if prefer_editor is not None:
        # Try the preferred editor first (match by name), then fall back.
        preferred = [(exe, name, fn) for (exe, name, fn) in candidates
                     if name == prefer_editor]
        ordered = preferred + [c for c in candidates if c[1] != prefer_editor]
    else:
        ordered = candidates

    for exe, name, fn in ordered:
        if shutil.which(exe):
            return name, fn

    return None, None


class OpenEditorClientMessageHandler:
    """Handle 'open-editor' messages sent by the browser client."""

    def __init__(self, prefer_editor=None, allowed_source_paths=None):
        self.allowed_source_paths = allowed_source_paths or {}
        self._editor_name, self._open_fn = _detect_editor(prefer_editor)
        if self._editor_name:
            logger.info(f"open-editor handler: detected editor '{self._editor_name}'")
        else:
            logger.warning("open-editor handler: no supported editor found on PATH")

    def __call__(self, msg):
        source_path = msg.get('source_path')
        if not source_path:
            logger.warning("open-editor: message missing 'source_path' field")
            return
        if source_path not in self.allowed_source_paths:
            logger.error(f"Cannot open ‘{source_path}’, not an allowed path")
            logger.debug("Allowed paths are: %s", json.dumps(self.allowed_source_paths))
            return
        fname = self.allowed_source_paths[source_path]
        line = msg.get('line')
        # sanitize inputs, make sure we're not exposed to some cheap injection
        # attack from the client
        if line is not None:
            line = int(line)
        col  = msg.get('col')
        if col is not None:
            col = int(col)
        msg_prefer = msg.get('prefer_editor')
        if msg_prefer is not None and msg_prefer != self._editor_name:
            editor_name, open_fn = _detect_editor(msg_prefer)
        else:
            editor_name, open_fn = self._editor_name, self._open_fn
        if open_fn is None:
            logger.warning("open-editor: no editor available, ignoring request")
            return
        logger.info(f"open-editor: opening {fname}:{line}:{col} in {editor_name}")
        open_fn(fname, line, col)


def make_hotreloader(
        computed_format, output,
        hotreload_server=None,
        allowed_source_paths=None,
    ):
    if computed_format != 'html':
        return HotReloaderDisabled()

    if hotreload_server is None:
        hotreload_server = HotReloaderServer('localhost', None)
        hotreload_server.start_server()

        # open editor handler:
        open_editor_handler = OpenEditorClientMessageHandler(
            allowed_source_paths=allowed_source_paths
        )
        hotreload_server.register_client_message_handler('open-editor', open_editor_handler)

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

    def new_run_send_error(self, error_info):
        pass

    def set_compiling_state(self, state):
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
            "action": 'update-main-content',
            "content_html": content_html,
        }

        self.hotreload_server.send_update_info(update_info)


    def new_run_send_error(self, error_info):

        update_info = {
            "action": 'error-display',
            "content_html": '<pre>' + html_escape(error_info['message']) + '</pre>',
        }
        
        self.hotreload_server.send_update_info(update_info)

    def set_compiling_state(self, state):
        update_info = {
            "action": 'set-compiling-state',
            "state": state,
        }
        self.hotreload_server.send_update_info(update_info)

