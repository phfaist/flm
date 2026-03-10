import logging
logger = logging.getLogger(__name__)

import re
import os.path
import json
import threading


import asyncio
import websockets.asyncio.server as websockets_server
# import websockets.exceptions as websockets_exceptions


from lxml import html

from .watch_util import find_available_port




def hotreload_js_code_to_inject(wsHost, wsPort):
    js_src = os.path.realpath(os.path.join(os.path.dirname(__file__), 'dist', 'watch_hotreload_inject.js'));
    with open(js_src, 'r', encoding='utf-8') as f:
        js_code = f.read()

    js_code = re.sub(r'^\/\/\# sourceMappingURL\=.*$', '', js_code, flags=re.MULTILINE)

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

        # update hot-reload clients
        update_info = {
            "action": 'update-main-content',
            "content_html": content_html,
        }

        self.hotreload_server.send_update_info(update_info)

        # save this as the current content
        self.previous_content = content


