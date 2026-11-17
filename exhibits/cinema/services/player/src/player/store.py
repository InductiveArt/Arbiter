import sqlite3
from pathlib import Path


class Store:
    """Resume points in one sqlite file: each member's second in each title."""

    def __init__(self, path: Path) -> None:
        # The connection may be used from one thread at a time, not only its opener.
        self._connection = sqlite3.connect(path, check_same_thread=False)
        _ = self._connection.execute(
            "CREATE TABLE IF NOT EXISTS progress ("
            "member TEXT NOT NULL, slug TEXT NOT NULL, second INTEGER NOT NULL, "
            "PRIMARY KEY (member, slug))"
        )
        self._connection.commit()

    def keep(self, member: str, slug: str, second: int) -> None:
        """Creates the resume point.

        Raises sqlite3.IntegrityError when one is already kept.
        """
        _ = self._connection.execute(
            "INSERT INTO progress (member, slug, second) VALUES (?, ?, ?)",
            (member, slug, second),
        )
        self._connection.commit()

    def at(self, member: str, slug: str) -> int | None:
        cursor = self._connection.execute(
            "SELECT second FROM progress WHERE member = ? AND slug = ?",
            (member, slug),
        )
        row: tuple[int] | None = cursor.fetchone()
        if row is None:
            return None
        return row[0]

    def move(self, member: str, slug: str, second: int) -> bool:
        cursor = self._connection.execute(
            "UPDATE progress SET second = ? WHERE member = ? AND slug = ?",
            (second, member, slug),
        )
        self._connection.commit()
        return cursor.rowcount > 0

    def clear(self, member: str, slug: str) -> bool:
        cursor = self._connection.execute(
            "DELETE FROM progress WHERE member = ? AND slug = ?",
            (member, slug),
        )
        self._connection.commit()
        return cursor.rowcount > 0

    def close(self) -> None:
        self._connection.close()
