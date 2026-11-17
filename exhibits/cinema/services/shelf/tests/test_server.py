import json
import threading
from http.client import HTTPConnection
from http.server import BaseHTTPRequestHandler
from http.server import HTTPServer
from pathlib import Path

from shelf.server import serve
from shelf.store import Store


def _neighbour(known: set[str]) -> HTTPServer:
    """A stub service answering 200 for the paths it knows and 404 otherwise."""

    class Stub(BaseHTTPRequestHandler):
        def do_GET(self) -> None:
            self.send_response(200 if self.path in known else 404)
            self.send_header("Content-Length", "0")
            self.end_headers()

        # The default handler narrates every request on stderr.
        def log_request(self, code: int | str = "-", size: int | str = "-") -> None:
            del code, size

    server = HTTPServer(("127.0.0.1", 0), Stub)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


def _world(tmp_path: Path, members: set[str], titles: set[str]) -> tuple[Store, list[HTTPServer]]:
    gate = _neighbour({f"/members/{member}" for member in members})
    catalog = _neighbour({f"/titles/{title}" for title in titles})
    store = Store(tmp_path / "shelf.db")
    server = serve(
        store,
        0,
        f"http://127.0.0.1:{gate.server_port}",
        f"http://127.0.0.1:{catalog.server_port}",
    )
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return store, [server, gate, catalog]


def _fold(store: Store, servers: list[HTTPServer]) -> None:
    for server in servers:
        server.shutdown()
        server.server_close()
    store.close()


def test_the_door_shelves_for_a_known_member_and_title(tmp_path: Path) -> None:
    store, servers = _world(tmp_path, {"ada"}, {"arrival"})
    door = HTTPConnection("127.0.0.1", servers[0].server_port)
    try:
        door.request("PUT", "/shelf/ada/arrival")
        assert door.getresponse().status == 204
        door.request("GET", "/shelf/ada")
        answer = door.getresponse()
        assert answer.status == 200
        shelved: object = json.loads(answer.read())
        assert shelved == {"titles": ["arrival"]}
    finally:
        door.close()
        _fold(store, servers)


def test_a_stranger_is_turned_away_at_the_door(tmp_path: Path) -> None:
    store, servers = _world(tmp_path, {"ada"}, {"arrival"})
    door = HTTPConnection("127.0.0.1", servers[0].server_port)
    try:
        door.request("PUT", "/shelf/mallory/arrival")
        turned = door.getresponse()
        assert turned.status == 401
        _ = turned.read()
        assert store.titles("mallory") == []
    finally:
        door.close()
        _fold(store, servers)


def test_a_title_the_catalog_does_not_carry_is_refused(tmp_path: Path) -> None:
    store, servers = _world(tmp_path, {"ada"}, {"arrival"})
    door = HTTPConnection("127.0.0.1", servers[0].server_port)
    try:
        door.request("PUT", "/shelf/ada/ghost")
        refused = door.getresponse()
        assert refused.status == 404
        _ = refused.read()
        assert store.titles("ada") == []
    finally:
        door.close()
        _fold(store, servers)
