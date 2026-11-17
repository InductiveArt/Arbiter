import os
from pathlib import Path

from shelf.server import serve
from shelf.store import Store


def main() -> None:
    place = Path(os.environ.get("SHELF_DATA", "shelf-data")) / "shelf.db"
    place.parent.mkdir(parents=True, exist_ok=True)
    store = Store(place)
    server = serve(
        store,
        int(os.environ.get("SHELF_PORT", "8540")),
        os.environ.get("GATE_URL", "http://127.0.0.1:8520"),
        os.environ.get("CATALOG_URL", "http://127.0.0.1:8510"),
    )
    try:
        server.serve_forever()
    finally:
        server.server_close()
        store.close()


if __name__ == "__main__":
    main()
