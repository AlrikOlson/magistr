"""Loopback-only HTTP adapter for the recorded inbox."""
import argparse
import json
import re
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlsplit

from store import Problem, Store

STATIC = Path(__file__).parent / "static"
ASSETS = {"/": ("index.html", "text/html; charset=utf-8"),
          "/app.js": ("app.js", "text/javascript; charset=utf-8"),
          "/style.css": ("style.css", "text/css; charset=utf-8")}


def make_server(path, port=0):
    store = Store(path)

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def send(self, status, data, content_type="application/json; charset=utf-8"):
            body = json.dumps(data).encode() if isinstance(data, (dict, list)) else data
            self.send_response(status)
            self.send_header("Content-Type", content_type)
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "no-store")
            self.send_header("X-Content-Type-Options", "nosniff")
            self.send_header("Content-Security-Policy", "default-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'")
            self.end_headers()
            self.wfile.write(body)

        def payload(self):
            if self.headers.get_content_type() != "application/json":
                raise Problem(415, "Send JSON content.")
            try:
                size = int(self.headers.get("Content-Length", "0"))
            except ValueError:
                raise Problem(400, "Invalid content length.")
            if not 0 < size <= 65536:
                raise Problem(413, "Send a message smaller than 64 KB.")
            try:
                body = json.loads(self.rfile.read(size))
            except (ValueError, UnicodeDecodeError):
                raise Problem(400, "The request is not valid JSON.")
            if not isinstance(body, dict):
                raise Problem(400, "Send a JSON object.")
            return body

        def dispatch(self):
            try:
                host = self.headers.get("Host", "")
                allowed_hosts = {f"127.0.0.1:{self.server.server_port}", f"localhost:{self.server.server_port}"}
                if host not in allowed_hosts:
                    raise Problem(403, "Use the local inbox address.")
                origin = self.headers.get("Origin")
                if origin and origin != f"http://{host}":
                    raise Problem(403, "Open this request from the local inbox.")
                url = urlsplit(self.path)
                query = {key: values[-1] for key, values in parse_qs(url.query).items()}
                match = re.fullmatch(r"/api/conversations/(\d+)", url.path)
                if self.command == "GET" and url.path == "/api/health":
                    return self.send(200, {"ready": True})
                if self.command == "GET" and url.path == "/api/conversations":
                    return self.send(200, store.list(**{key: query.get(key, "") for key in ("q", "status", "assignee")}))
                if self.command == "GET" and match:
                    return self.send(200, store.get(int(match[1])))
                if self.command == "POST" and url.path == "/api/mail":
                    item, created = store.receive(self.payload())
                    return self.send(201 if created else 200, {"created": created, "conversation": item})
                if self.command == "PATCH" and match:
                    return self.send(200, store.update(int(match[1]), self.payload()))
                if self.command == "GET" and url.path in ASSETS:
                    filename, mime = ASSETS[url.path]
                    return self.send(200, (STATIC / filename).read_bytes(), mime)
                raise Problem(404, "Page not found.")
            except Problem as error:
                self.send(error.status, {"error": str(error)})

        do_GET = dispatch
        do_POST = dispatch
        do_PATCH = dispatch

    return ThreadingHTTPServer(("127.0.0.1", port), Handler)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--db", default="inbox.sqlite")
    parser.add_argument("--port", type=int, default=8765)
    arguments = parser.parse_args()
    server = make_server(arguments.db, arguments.port)
    print(f"Inbox ready at http://127.0.0.1:{server.server_port}", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
