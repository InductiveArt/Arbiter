import json
import threading
from http.client import HTTPConnection
from pathlib import Path

from tickets.server import serve
from tickets.store import Store


def test_the_door_serves_a_rental_round_trip(tmp_path: Path) -> None:
    store = Store(tmp_path / "tickets.db")
    server = serve(store, 0)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    door = HTTPConnection("127.0.0.1", server.server_port)
    try:
        door.request("PUT", "/tickets/ada/arrival", json.dumps({"nights": 2}).encode())
        assert door.getresponse().status == 204
        door.request("GET", "/tickets/ada/arrival")
        answer = door.getresponse()
        assert answer.status == 200
        standing: object = json.loads(answer.read())
        assert standing == {"nights": 2}
        door.request("DELETE", "/tickets/ada/arrival")
        assert door.getresponse().status == 204
        door.request("GET", "/tickets/ada/arrival")
        gone = door.getresponse()
        assert gone.status == 404
        _ = gone.read()
    finally:
        door.close()
        server.shutdown()
        server.server_close()
        store.close()
