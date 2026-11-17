import os
from pathlib import Path

from player.server import serve
from player.store import Store


def main() -> None:
    place = Path(os.environ.get("PLAYER_DATA", "player-data")) / "progress.db"
    place.parent.mkdir(parents=True, exist_ok=True)
    store = Store(place)
    server = serve(
        store,
        int(os.environ.get("PLAYER_PORT", "8500")),
        os.environ.get("TICKETS_URL", "http://127.0.0.1:8530"),
        os.environ.get("CATALOG_URL", "http://127.0.0.1:8510"),
    )
    try:
        server.serve_forever()
    finally:
        server.server_close()
        store.close()


if __name__ == "__main__":
    main()
