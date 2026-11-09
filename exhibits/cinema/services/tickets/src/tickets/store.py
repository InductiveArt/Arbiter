import sqlite3
from pathlib import Path


class Store:
    """Rentals in one sqlite file: each member's nights on each title."""

    def __init__(self, path: Path) -> None:
        # The connection may be used from one thread at a time, not only its opener.
        self._connection = sqlite3.connect(path, check_same_thread=False)
        _ = self._connection.execute(
            "CREATE TABLE IF NOT EXISTS tickets ("
            "member TEXT NOT NULL, slug TEXT NOT NULL, nights INTEGER NOT NULL, "
            "PRIMARY KEY (member, slug))"
        )
        self._connection.commit()

    def rent(self, member: str, slug: str, nights: int) -> None:
        """Creates the rental.

        Raises sqlite3.IntegrityError when the rental already stands.
        """
        _ = self._connection.execute(
            "INSERT INTO tickets (member, slug, nights) VALUES (?, ?, ?)",
            (member, slug, nights),
        )
        self._connection.commit()

    def standing(self, member: str, slug: str) -> int | None:
        cursor = self._connection.execute(
            "SELECT nights FROM tickets WHERE member = ? AND slug = ?",
            (member, slug),
        )
        row: tuple[int] | None = cursor.fetchone()
        if row is None:
            return None
        return row[0]

    def extend(self, member: str, slug: str, nights: int) -> bool:
        cursor = self._connection.execute(
            "UPDATE tickets SET nights = ? WHERE member = ? AND slug = ?",
            (nights, member, slug),
        )
        self._connection.commit()
        return cursor.rowcount > 0

    def cancel(self, member: str, slug: str) -> bool:
        cursor = self._connection.execute(
            "DELETE FROM tickets WHERE member = ? AND slug = ?",
            (member, slug),
        )
        self._connection.commit()
        return cursor.rowcount > 0

    def close(self) -> None:
        self._connection.close()
