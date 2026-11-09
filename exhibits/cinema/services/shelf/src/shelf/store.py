import sqlite3
from pathlib import Path


class Store:
    """Watchlists in one sqlite file: the titles each member shelved."""

    def __init__(self, path: Path) -> None:
        # The connection may be used from one thread at a time, not only its opener.
        self._connection = sqlite3.connect(path, check_same_thread=False)
        _ = self._connection.execute(
            "CREATE TABLE IF NOT EXISTS shelf ("
            "member TEXT NOT NULL, slug TEXT NOT NULL, "
            "PRIMARY KEY (member, slug))"
        )
        self._connection.commit()

    def shelve(self, member: str, slug: str) -> bool:
        cursor = self._connection.execute(
            "INSERT OR IGNORE INTO shelf (member, slug) VALUES (?, ?)",
            (member, slug),
        )
        self._connection.commit()
        return cursor.rowcount > 0

    def titles(self, member: str) -> list[str]:
        cursor = self._connection.execute(
            "SELECT slug FROM shelf WHERE member = ? ORDER BY slug",
            (member,),
        )
        rows: list[tuple[str]] = cursor.fetchall()
        return [row[0] for row in rows]

    def unshelve(self, member: str, slug: str) -> bool:
        cursor = self._connection.execute(
            "DELETE FROM shelf WHERE member = ? AND slug = ?",
            (member, slug),
        )
        self._connection.commit()
        return cursor.rowcount > 0

    def close(self) -> None:
        self._connection.close()
