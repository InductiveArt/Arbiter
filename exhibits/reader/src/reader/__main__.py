import os
from pathlib import Path

from reader.server import serve
from reader.store import Store


def main() -> None:
    place = Path(os.environ.get("READER_DATA", "reader-data")) / "marks.db"
    place.parent.mkdir(parents=True, exist_ok=True)
    store = Store(place)
    server = serve(store, int(os.environ.get("READER_PORT", "8400")))
    try:
        server.serve_forever()
    finally:
        server.server_close()
        store.close()


if __name__ == "__main__":
    main()
