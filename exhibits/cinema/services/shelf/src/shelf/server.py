import json
from http.client import HTTPConnection
from http.server import BaseHTTPRequestHandler
from http.server import HTTPServer
from urllib.parse import unquote
from urllib.parse import urlsplit

from shelf.store import Store


def _status(base: str, path: str) -> int:
    """GETs the path behind the base URL; 502 when nothing answers there."""
    place = urlsplit(base)
    door = HTTPConnection(place.hostname or "", place.port or 80, timeout=2)
    try:
        door.request("GET", path)
        answer = door.getresponse()
        _ = answer.read()
    except OSError:
        return 502
    finally:
        door.close()
    return answer.status


def _shelf_path(path: str) -> tuple[str, str] | None:
    parts = [unquote(part) for part in path.strip("/").split("/")]
    if len(parts) == 3 and parts[0] == "shelf" and all(parts):
        return parts[1], parts[2]
    return None


def _member_path(path: str) -> str | None:
    parts = [unquote(part) for part in path.strip("/").split("/")]
    if len(parts) == 2 and parts[0] == "shelf" and all(parts):
        return parts[1]
    return None


def _put(store: Store, gate: str, catalog: str, path: str) -> int:
    place = _shelf_path(path)
    if place is None:
        return 400
    member, slug = place
    if _status(gate, f"/members/{member}") != 200:
        return 401
    if _status(catalog, f"/titles/{slug}") != 200:
        return 404
    _ = store.shelve(member, slug)
    return 204


def _get(store: Store, path: str) -> tuple[int, bytes]:
    member = _member_path(path)
    if member is not None:
        return 200, json.dumps({"titles": store.titles(member)}).encode()
    place = _shelf_path(path)
    if place is None:
        return 400, b""
    if place[1] in store.titles(place[0]):
        return 204, b""
    return 404, b""


def _delete(store: Store, path: str) -> int:
    place = _shelf_path(path)
    if place is None:
        return 400
    return 204 if store.unshelve(*place) else 404


def serve(store: Store, port: int, gate: str, catalog: str) -> HTTPServer:
    """Builds the server for the store; gate and catalog are the base URLs it dials."""

    class Handler(BaseHTTPRequestHandler):
        def do_PUT(self) -> None:
            length: str | None = self.headers.get("Content-Length")
            _ = self.rfile.read(int(length or 0))
            self._answer(_put(store, gate, catalog, self.path))

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
