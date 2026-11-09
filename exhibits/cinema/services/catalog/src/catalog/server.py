import json
from http.server import BaseHTTPRequestHandler
from http.server import HTTPServer
from urllib.parse import unquote

from catalog.store import Store


def _title_path(path: str) -> str | None:
    parts = [unquote(part) for part in path.strip("/").split("/")]
    if len(parts) == 2 and parts[0] == "titles" and all(parts):
        return parts[1]
    return None


def _title_of(body: bytes) -> tuple[str, int] | None:
    try:
        parsed: object = json.loads(body)
    except ValueError:
        return None
    match parsed:
        case {"name": str(name), "minutes": int(minutes)}:
            return name, minutes
        case _:
            return None


def _put(store: Store, path: str, body: bytes) -> int:
    slug = _title_path(path)
    title = _title_of(body)
    if slug is None or title is None:
        return 400
    name, minutes = title
    if not store.correct(slug, name, minutes):
        store.add(slug, name, minutes)
    return 204


def _get(store: Store, path: str) -> tuple[int, bytes]:
    slug = _title_path(path)
    if slug is None:
        return 400, b""
    title = store.carried(slug)
    if title is None:
        return 404, b""
    name, minutes = title
    return 200, json.dumps({"name": name, "minutes": minutes}).encode()


def _delete(store: Store, path: str) -> int:
    slug = _title_path(path)
    if slug is None:
        return 400
    return 204 if store.withdraw(slug) else 404


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
