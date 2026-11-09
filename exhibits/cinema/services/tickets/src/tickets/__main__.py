import os
from pathlib import Path

from tickets.server import serve
from tickets.store import Store


def main() -> None:
    place = Path(os.environ.get("TICKETS_DATA", "tickets-data")) / "tickets.db"
    place.parent.mkdir(parents=True, exist_ok=True)
    store = Store(place)
    server = serve(store, int(os.environ.get("TICKETS_PORT", "8530")))
    try:
        server.serve_forever()
    finally:
        server.server_close()
        store.close()


if __name__ == "__main__":
    main()
