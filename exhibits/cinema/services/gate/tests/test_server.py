import json
import threading
from http.client import HTTPConnection
from pathlib import Path

from gate.server import serve
from gate.store import Store


def test_the_door_serves_a_member_round_trip(tmp_path: Path) -> None:
    store = Store(tmp_path / "members.db")
    server = serve(store, 0)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    door = HTTPConnection("127.0.0.1", server.server_port)
    try:
        door.request("PUT", "/members/ada", json.dumps({"name": "Ada Lovelace"}).encode())
        assert door.getresponse().status == 204
        door.request("GET", "/members/ada")
        answer = door.getresponse()
        assert answer.status == 200
        body: object = json.loads(answer.read())
        assert body == {"name": "Ada Lovelace"}
        door.request("DELETE", "/members/ada")
        assert door.getresponse().status == 204
        door.request("GET", "/members/ada")
        gone = door.getresponse()
        assert gone.status == 404
        _ = gone.read()
    finally:
        door.close()
        server.shutdown()
        server.server_close()
        store.close()
