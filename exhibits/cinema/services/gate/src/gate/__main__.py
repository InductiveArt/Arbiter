import os
from pathlib import Path

from gate.server import serve
from gate.store import Store


def main() -> None:
    place = Path(os.environ.get("GATE_DATA", "gate-data")) / "members.db"
    place.parent.mkdir(parents=True, exist_ok=True)
    store = Store(place)
    server = serve(store, int(os.environ.get("GATE_PORT", "8520")))
    try:
        server.serve_forever()
    finally:
        server.server_close()
        store.close()


if __name__ == "__main__":
    main()
