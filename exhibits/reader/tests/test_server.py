import json
import threading
from http.client import HTTPConnection
from pathlib import Path

from reader.server import serve
from reader.store import Store


def test_the_door_serves_a_mark_round_trip(tmp_path: Path) -> None:
    store = Store(tmp_path / "marks.db")
    server = serve(store, 0)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    door = HTTPConnection("127.0.0.1", server.server_port)
    try:
        door.request("PUT", "/marks/ada/sicp", json.dumps({"page": 12}).encode())
        assert door.getresponse().status == 204
        door.request("GET", "/marks/ada/sicp")
        answer = door.getresponse()
        assert answer.status == 200
        body: object = json.loads(answer.read())
        assert body == {"page": 12}
        door.request("DELETE", "/marks/ada/sicp")
        assert door.getresponse().status == 204
        door.request("GET", "/marks/ada/sicp")
        gone = door.getresponse()
        assert gone.status == 404
        _ = gone.read()
    finally:
        door.close()
        server.shutdown()
        server.server_close()
        store.close()
