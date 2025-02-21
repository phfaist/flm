import logging
logger = logging.getLogger(__name__)

import json
import tempfile
import os
import os.path
import mimetypes
import time

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import threading
import socket
import errno
import asyncio
import websockets.asyncio.server as websockets_server
import websockets.exceptions as websockets_exceptions

import watchfiles

logging.getLogger("watchfiles").setLevel(logging.WARNING)
logging.getLogger("websockets").setLevel(logging.WARNING)

from pylatexenc.latexnodes import LatexWalkerLocatedError

from . import main


ext_by_format = {
    "html": ".html",
    "latex": ".tex",
}


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



def find_available_port(host="localhost", base_port=8000, maxcount=64):
    """Find a port not in ues starting at given port"""
    count = 0
    while count <= maxcount:
        port = base_port + count
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
            if s.connect_ex((host, port)) == 0:
                continue
            else:
                return port
        count += 1
    raise RuntimeError(f"Couldn't find a free port within {maxcount} of {base_port} on {host}")



def main_watch(**kwargs):

    with tempfile.TemporaryDirectory() as temp_dir:

        temp_dir = os.path.realpath(temp_dir)

        #arg_files = kwargs.get('files', None)
        arg_output = kwargs.get('output', None)

        if arg_output is not None:
            raise ValueError(
                "Watch mode starts its own server, please do not provide an output file "
                "with watch mode (no -o option)"
            )

        #
        # inspect the run meta-information without running yet.
        #

        main_runner = main.Main(**kwargs)
        run_object = main_runner.make_run_object()
        
        # figure out what the output format is (possibly computed from workflow)
        computed_format = run_object.wenv.workflow.fragment_renderer_information.format_name

        computed_main_input_dir = os.path.dirname(main_runner.flm_run_info['input_source'])

        new_arg_output = os.path.join(
            temp_dir,
            'index' + ext_by_format.get(computed_format, '.'+computed_format)
        )

        run_kwargs = dict(kwargs, output=new_arg_output)

        #
        # Run the full procedure a first time. Don't reuse main_runner or
        # run_object since we might have changed some of the options,
        # e.g. output file.  Let's be safe.
        #

        server = None

        ws_notify_update_queue = asyncio.Queue()

        enable_hotreload = (computed_format == 'html')

        def do_compile(do_ws_update=True):

            info = main.main(**run_kwargs)

            if enable_hotreload and do_ws_update and computed_format == 'html':
                # get the body of the newly generated HTML, to inform
                # hotreload clients
                with open(new_arg_output, 'r', encoding='utf-8') as f:
                    content = f.read()

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
                logger.info("Preparing to send update to clients.")
                asyncio.run_coroutine_threadsafe(
                    ws_notify_update_queue.put(json.dumps(update_info)),
                    ws_loop_info['loop'],
                )

            logger.info("🚀 Document recompiled successfully 🚀")

            return info

        info = do_compile(do_ws_update=False) #main.main(**run_kwargs)

        watch_files = [
            info['flm_run_info']['input_source']
        ]

        if info['result_info']['content_parts_infos']:
            watch_files += [
                cpinfo['input_source']
                for cpinfo in info['result_info']['content_parts_infos']['parts']
            ]

        logger.info('Successfully compiled the FLM document.')


        #
        # Now start the WEB server.
        #
        class FlmRunServer(BaseHTTPRequestHandler):
            def do_GET(self):
                path = self.path
                if path.endswith('/'):
                    path += 'index.html'

                path = path.lstrip('/')
                fullpath = os.path.realpath(os.path.join(temp_dir, path))

                if not fullpath.startswith(temp_dir):
                    self.send_response(404)
                    self.wfile.write(bytes("<html><body>404</body></html>", 'utf-8'))
                    return

                try:
                    
                    with open(fullpath, 'rb') as f:
                        self.send_response(200)
                        (mime_type, mime_encoding) = mimetypes.guess_type('file://'+fullpath)
                        if mime_type is not None:
                            self.send_header("Content-Type", mime_type)
                        self.end_headers()
                        self.wfile.write(f.read())

                except IOError:
                    self.send_response(404)
                    self.wfile.write(bytes(
                        f"<html><body>404 not found: {path}</body></html>",
                        'utf-8'
                    ))
                    return

                self.end_headers()
                return

        server_hostname = 'localhost'
        server_port = find_available_port(server_hostname, 18910)
        server = ThreadingHTTPServer((server_hostname, server_port), FlmRunServer)

        server_thread = threading.Thread(target=server.serve_forever)
        server_thread.daemon = True
        server_thread.start()

        #
        # Now, start the WebSockets server for hot-reloading pages.
        #

        ws_host = 'localhost'
        ws_port = find_available_port(ws_host, 28102)

        ws_connected_clients = []
        ws_loop_info = { "loop": None }

        async def ws_notify_update_clients():
            while True:
                # Wait for a message on the Queue
                message = await ws_notify_update_queue.get()
                if ws_connected_clients:
                    # Send to all connected clients
                    await asyncio.gather(*[
                        client.send(message)
                        for client in ws_connected_clients
                    ])
                    logger.info(f"Sent update to {len(ws_connected_clients)} clients")

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
            ws_server = await websockets_server.serve(ws_handler, ws_host, ws_port)
            logger.info(f"Websocket server started for automatic hot-reload")
            # on {ws_host}:{ws_port}

            await asyncio.gather(*[
                ws_server.wait_closed(), ws_notify_update_clients(),
            ])

        def ws_start_server_in_thread():
            ws_loop = asyncio.new_event_loop()
            ws_loop_info["loop"] = ws_loop
            asyncio.set_event_loop(ws_loop)
            ws_loop.run_until_complete( ws_async_start_and_serve() )


        ws_server_thread = threading.Thread(target=ws_start_server_in_thread, daemon=True)
        ws_server_thread.start()


        #
        # Inject hot-reload code in generate file.
        #

        def inject_hotreload(fname):
            with open(fname, 'r', encoding='utf-8') as f:
                content = f.read()

            # inject hot-reload code in generated HTML, if necessary
            ibodyend = content.rfind('</body>')
            if ibodyend == -1:
                logger.warning("Couldn't inject hot-reload JS code, didn't find </body>")
                return

            hotreload_js = (
                """<script type="text/javascript">"""
                + hotreload_js_code_to_inject(ws_host, ws_port)
                + """</script>"""
            )

            content = (
                content[:ibodyend]
                + hotreload_js
                + content[ibodyend:]
            )

            with open(fname, 'w', encoding='utf-8') as fw:
                fw.write(content)

        # do it!
        inject_hotreload(new_arg_output)


        logger.info(f"""
**************************************************

       🌎 Server started 🌎

  ==>  http://{server_hostname}:{server_port}/  <==

  Point your browser to this address to
  view the compiled document.

**************************************************
""")
        logger.info('Watching input files, hit Interrupt (Ctrl+C) to quit.')

        try:

            for changes in watchfiles.watch(*watch_files, raise_interrupt=False, debounce=2000):

                logger.info('Input file(s) changed: %s', ",".join([
                    os.path.relpath(c[1], computed_main_input_dir)
                    for c in changes
                ]))

                try:
                    do_compile(do_ws_update=True)
                    inject_hotreload(new_arg_output)

                except LatexWalkerLocatedError as e:
                    logger.error("Error!\n\n%s\n", e)

                except Exception as e:
                    logger.error("Error recompiling document! %s", e, exc_info=e)

            logger.info('Okay, quitting now.')

        finally:
            logger.info('Shutting down server.')
            server.shutdown()




