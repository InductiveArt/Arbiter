import json
from http.client import HTTPConnection
from http.server import BaseHTTPRequestHandler
from http.server import HTTPServer
from urllib.parse import unquote
from urllib.parse import urlsplit

from player.store import Store


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


def _marked(path: str, head: str) -> tuple[str, str] | None:
    parts = [unquote(part) for part in path.strip("/").split("/")]
    if len(parts) == 3 and parts[0] == head and all(parts):
        return parts[1], parts[2]
    return None


def _second_of(body: bytes) -> int | None:
    try:
        parsed: object = json.loads(body)
    except ValueError:
        return None
    match parsed:
        case {"second": int(second)}:
            return second
        case _:
            return None


def _put(store: Store, path: str, body: bytes) -> int:
    place = _marked(path, "progress")
    second = _second_of(body)
    if place is None or second is None:
        return 400
    member, slug = place
    if not store.move(member, slug, second):
        store.keep(member, slug, second)
    return 204


def _play(store: Store, tickets: str, catalog: str, place: tuple[str, str]) -> tuple[int, bytes]:
    member, slug = place
    if _status(tickets, f"/tickets/{member}/{slug}") != 200:
        return 403, b""
    if _status(catalog, f"/titles/{slug}") != 200:
        return 404, b""
    second = store.at(member, slug)
    if second is None:
        second = 0
    return 200, json.dumps({"stream": slug, "at": second}).encode()


def _get(store: Store, tickets: str, catalog: str, path: str) -> tuple[int, bytes]:
    played = _marked(path, "play")
    if played is not None:
        return _play(store, tickets, catalog, played)
    place = _marked(path, "progress")
    if place is None:
        return 400, b""
    second = store.at(*place)
    if second is None:
        return 404, b""
    return 200, json.dumps({"second": second}).encode()


def _delete(store: Store, path: str) -> int:
    place = _marked(path, "progress")
    if place is None:
        return 400
    return 204 if store.clear(*place) else 404


def serve(store: Store, port: int, tickets: str, catalog: str) -> HTTPServer:
    """Builds the server for the store; tickets and catalog are the base URLs it dials."""

    class Handler(BaseHTTPRequestHandler):
        def do_PUT(self) -> None:
            length: str | None = self.headers.get("Content-Length")
            body = self.rfile.read(int(length or 0))
            self._answer(_put(store, self.path, body))

        def do_GET(self) -> None:
            status, body = _get(store, tickets, catalog, self.path)
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
