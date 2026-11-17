import sqlite3
from pathlib import Path


class Store:
    """Bookmarks in one sqlite file: each reader's page in each book."""

    def __init__(self, path: Path) -> None:
        # The connection may be used from one thread at a time, not only its opener.
        self._connection = sqlite3.connect(path, check_same_thread=False)
        _ = self._connection.execute(
            "CREATE TABLE IF NOT EXISTS bookmarks ("
            "reader TEXT NOT NULL, book TEXT NOT NULL, page INTEGER NOT NULL, "
            "PRIMARY KEY (reader, book))"
        )
        self._connection.commit()

    def mark(self, reader: str, book: str, page: int) -> None:
        """Creates the bookmark.

        Raises sqlite3.IntegrityError when the bookmark already exists.
        """
        _ = self._connection.execute(
            "INSERT INTO bookmarks (reader, book, page) VALUES (?, ?, ?)",
            (reader, book, page),
        )
        self._connection.commit()

    def where(self, reader: str, book: str) -> int | None:
        cursor = self._connection.execute(
            "SELECT page FROM bookmarks WHERE reader = ? AND book = ?",
            (reader, book),
        )
        row: tuple[int] | None = cursor.fetchone()
        if row is None:
            return None
        return row[0]

    def move(self, reader: str, book: str, page: int) -> bool:
        cursor = self._connection.execute(
            "UPDATE bookmarks SET page = ? WHERE reader = ? AND book = ?",
            (page, reader, book),
        )
        self._connection.commit()
        return cursor.rowcount > 0

    def forget(self, reader: str, book: str) -> bool:
        cursor = self._connection.execute(
            "DELETE FROM bookmarks WHERE reader = ? AND book = ?",
            (reader, book),
        )
        self._connection.commit()
        return cursor.rowcount > 0

    def close(self) -> None:
        self._connection.close()
