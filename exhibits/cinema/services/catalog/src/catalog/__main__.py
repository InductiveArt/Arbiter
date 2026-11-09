import os
from pathlib import Path

from catalog.server import serve
from catalog.store import Store


def main() -> None:
    place = Path(os.environ.get("CATALOG_DATA", "catalog-data")) / "titles.db"
    place.parent.mkdir(parents=True, exist_ok=True)
    store = Store(place)
    server = serve(store, int(os.environ.get("CATALOG_PORT", "8510")))
    try:
        server.serve_forever()
    finally:
        server.server_close()
        store.close()


if __name__ == "__main__":
    main()
