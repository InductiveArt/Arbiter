import json
from http.server import BaseHTTPRequestHandler
from http.server import HTTPServer
from urllib.parse import unquote

from gate.store import Store


def _member_path(path: str) -> str | None:
    parts = [unquote(part) for part in path.strip("/").split("/")]
    if len(parts) == 2 and parts[0] == "members" and all(parts):
        return parts[1]
    return None


def _name_of(body: bytes) -> str | None:
    try:
        parsed: object = json.loads(body)
    except ValueError:
        return None
    match parsed:
        case {"name": str(name)}:
            return name
        case _:
            return None


def _put(store: Store, path: str, body: bytes) -> int:
    handle = _member_path(path)
    name = _name_of(body)
    if handle is None or name is None:
        return 400
    if not store.rename(handle, name):
        store.enroll(handle, name)
    return 204


def _get(store: Store, path: str) -> tuple[int, bytes]:
    handle = _member_path(path)
    if handle is None:
        return 400, b""
    name = store.named(handle)
    if name is None:
        return 404, b""
    return 200, json.dumps({"name": name}).encode()


def _delete(store: Store, path: str) -> int:
    handle = _member_path(path)
    if handle is None:
        return 400
    return 204 if store.strike(handle) else 404


def serve(store: Store, port: int) -> HTTPServer:
    """Builds the server for the store; the caller runs and shuts it down."""

    class Handler(BaseHTTPRequestHandler):
        def do_PUT(self) -> None:
            length: str | None = self.headers.get("Content-Length")
            body = self.rfile.read(int(length or 0))
            self._answer(_put(store, self.path, body))

        def do_GET(self) -> None:
            status, body = _get(store, self.path)
            self._answer(status, body)

        def do_DELETE(self) -> None:
            self._answer(_delete(store, self.path))

        def _answer(self, status: int, body: bytes = b"") -> None:
            self.send_response(status)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            _ = self.wfile.write(body)

        # The default handler narrates every request on stderr.
        def log_request(self, code: int | str = "-", size: int | str = "-") -> None:
            del code, size

    return HTTPServer(("127.0.0.1", port), Handler)
