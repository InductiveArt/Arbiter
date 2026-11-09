import json
import threading
from http.client import HTTPConnection
from http.server import BaseHTTPRequestHandler
from http.server import HTTPServer
from pathlib import Path

from player.server import serve
from player.store import Store


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


def _world(tmp_path: Path, rentals: set[str], titles: set[str]) -> tuple[Store, list[HTTPServer]]:
    tickets = _neighbour({f"/tickets/{rental}" for rental in rentals})
    catalog = _neighbour({f"/titles/{title}" for title in titles})
    store = Store(tmp_path / "progress.db")
    server = serve(
        store,
        0,
        f"http://127.0.0.1:{tickets.server_port}",
        f"http://127.0.0.1:{catalog.server_port}",
    )
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return store, [server, tickets, catalog]


def _fold(store: Store, servers: list[HTTPServer]) -> None:
    for server in servers:
        server.shutdown()
        server.server_close()
    store.close()


def test_a_stream_opens_against_a_standing_rental(tmp_path: Path) -> None:
    store, servers = _world(tmp_path, {"ada/arrival"}, {"arrival"})
    store.keep("ada", "arrival", 300)
    door = HTTPConnection("127.0.0.1", servers[0].server_port)
    try:
        door.request("GET", "/play/ada/arrival")
        answer = door.getresponse()
        assert answer.status == 200
        opened: object = json.loads(answer.read())
        assert opened == {"stream": "arrival", "at": 300}
    finally:
        door.close()
        _fold(store, servers)


def test_an_unrented_title_refuses_to_play(tmp_path: Path) -> None:
    store, servers = _world(tmp_path, {"ada/arrival"}, {"arrival"})
    door = HTTPConnection("127.0.0.1", servers[0].server_port)
    try:
        door.request("GET", "/play/mallory/arrival")
        refused = door.getresponse()
        assert refused.status == 403
        _ = refused.read()
    finally:
        door.close()
        _fold(store, servers)


def test_an_unknown_title_refuses_to_play(tmp_path: Path) -> None:
    store, servers = _world(tmp_path, {"ada/ghost"}, {"arrival"})
    door = HTTPConnection("127.0.0.1", servers[0].server_port)
    try:
        door.request("GET", "/play/ada/ghost")
        refused = door.getresponse()
        assert refused.status == 404
        _ = refused.read()
    finally:
        door.close()
        _fold(store, servers)


def test_the_door_keeps_a_resume_point_round_trip(tmp_path: Path) -> None:
    store, servers = _world(tmp_path, set(), set())
    door = HTTPConnection("127.0.0.1", servers[0].server_port)
    try:
        door.request("PUT", "/progress/ada/arrival", json.dumps({"second": 300}).encode())
        assert door.getresponse().status == 204
        door.request("GET", "/progress/ada/arrival")
        answer = door.getresponse()
        assert answer.status == 200
        kept: object = json.loads(answer.read())
        assert kept == {"second": 300}
        door.request("DELETE", "/progress/ada/arrival")
        assert door.getresponse().status == 204
        door.request("GET", "/progress/ada/arrival")
        gone = door.getresponse()
        assert gone.status == 404
        _ = gone.read()
    finally:
        door.close()
        _fold(store, servers)
