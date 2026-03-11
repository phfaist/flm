import logging
logger = logging.getLogger(__name__)

import json
import tempfile
import os
import os.path
import mimetypes

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import threading
import socket
# import errno

import watchfiles

logging.getLogger("watchfiles").setLevel(logging.WARNING)
logging.getLogger("websockets").setLevel(logging.WARNING)

from pylatexenc.latexnodes import LatexWalkerLocatedError

from . import main
from .run import ResourceAccessorBase


from .watch_util import find_available_port
from .watch_hotreload import make_hotreloader



ext_by_format = {
    "html": ".html",
    "latex": ".tex",
}





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

        def include_node_data_attrs_fn_src_line_col(node, when, **kwargs):
            source_path = None
            latex_walker = node.latex_walker
            if (latex_walker is not None
                and latex_walker.resource_info is not None
                and hasattr(latex_walker.resource_info, 'source_path')):
                source_path = latex_walker.resource_info.source_path
                cwd = ResourceAccessorBase.get_cwd_for_resource_info(
                    latex_walker.resource_info,
                    main_runner.flm_run_info
                )
                if cwd is not None:
                    source_path = os.path.join(cwd, source_path)
            line, col = latex_walker.pos_to_lineno_colno(node.pos)
            #logger.debug(f"source ({source_path}, {line}, {col}) for {repr(node)}.")
            return { 'sourcepath': json.dumps([source_path,line,col]) }

        inline_config = None
        if computed_format == 'html':
            inline_config = { 'flm': {
                'renderer': {
                    'html': {
                        'include_node_data_attrs_fn': include_node_data_attrs_fn_src_line_col,
                    }
                }
            } }


        run_kwargs = dict(kwargs, output=new_arg_output, inline_config=inline_config)

        #
        # Run the full procedure a first time. Don't reuse main_runner or
        # run_object since we might have changed some of the options,
        # e.g. output file.  Let's be safe.
        #

        server = None

        def do_compile(hotreloader=None):

            #
            # Compile - main run NOW!
            #
            try:
                info = main.main(**run_kwargs)
            except LatexWalkerLocatedError as e:
                error_info = {'message': str(e), 'exc': e}
                if hotreloader is not None and hotreloader.is_enabled():
                    hotreloader.new_run_send_error(error_info)
                raise
            except Exception as e:
                error_info = {'message': str(e), 'exc': e}
                if hotreloader is not None and hotreloader.is_enabled():
                    hotreloader.new_run_send_error(error_info)
                raise

            if hotreloader is not None and hotreloader.is_enabled():
                # Inform our hotreloader of a new run.
                with open(new_arg_output, 'r', encoding='utf-8') as f:
                    content = f.read()

                # Send update to our clients
                hotreloader.new_run_send_update(info=info, content=content)

            logger.info("🚀 Document recompiled successfully 🚀")

            return info

        # first run does not have any hotreloader set, no existing client to update yet.
        info = do_compile(hotreloader=None)

        watch_files = [
            info['flm_run_info']['input_source']
        ]

        if info['result_info']['content_parts_infos']:
            watch_files += [
                cpinfo['input_source']
                for cpinfo in info['result_info']['content_parts_infos']['parts']
                if cpinfo['input_source']
            ]

        logger.info('Successfully compiled the FLM document.')

        # store the known sources (the files we're watching), so that we know
        # which files we're allowed to open in an editor.
        allowed_source_paths = {
            fn: os.path.abspath(fn)
            for fn in watch_files
        }


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
                    logger.warning(
                        'Requested path %r is invalid as fullpath=%r does not start '
                        'with temp_dir=%r',
                        path, fullpath, temp_dir
                    )
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

                except IOError as e:

                    # Don't produce warning for some files that browsers tend to
                    # try to fetch on their own
                    if path not in ('favicon.ico', ):
                        logger.warning(
                            'Could not retrieve requested path %r: %s',
                            path, e
                        )

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

        hotreloader = make_hotreloader(
            computed_format=computed_format,
            output=new_arg_output,
            allowed_source_paths=allowed_source_paths,
        )


        # do it!
        hotreloader.inject_hotreload_js()


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
                    hotreloader.set_compiling_state('compiling')
                    do_compile(hotreloader=hotreloader)
                    hotreloader.inject_hotreload_js()

                except LatexWalkerLocatedError as e:
                    logger.error("Error!\n\n%s\n", e)

                except Exception as e:
                    logger.error("Error recompiling document! %s", e, exc_info=e)

                finally:
                    hotreloader.set_compiling_state('idle')

            logger.info('Okay, quitting now.')

        finally:
            logger.info('Shutting down server.')
            server.shutdown()




