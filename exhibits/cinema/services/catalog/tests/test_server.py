import json
import threading
from http.client import HTTPConnection
from pathlib import Path

from catalog.server import serve
from catalog.store import Store


def test_the_door_serves_a_title_round_trip(tmp_path: Path) -> None:
    store = Store(tmp_path / "titles.db")
    server = serve(store, 0)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    door = HTTPConnection("127.0.0.1", server.server_port)
    try:
        body = json.dumps({"name": "Arrival", "minutes": 116}).encode()
        door.request("PUT", "/titles/arrival", body)
        assert door.getresponse().status == 204
        door.request("GET", "/titles/arrival")
        answer = door.getresponse()
        assert answer.status == 200
        carried: object = json.loads(answer.read())
        assert carried == {"name": "Arrival", "minutes": 116}
        door.request("DELETE", "/titles/arrival")
        assert door.getresponse().status == 204
        door.request("GET", "/titles/arrival")
        gone = door.getresponse()
        assert gone.status == 404
        _ = gone.read()
    finally:
        door.close()
        server.shutdown()
        server.server_close()
        store.close()
