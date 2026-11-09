import sqlite3
from pathlib import Path


class Store:
    """Titles in one sqlite file: each slug with its name and minutes."""

    def __init__(self, path: Path) -> None:
        # The connection may be used from one thread at a time, not only its opener.
        self._connection = sqlite3.connect(path, check_same_thread=False)
        _ = self._connection.execute(
            "CREATE TABLE IF NOT EXISTS titles ("
            "slug TEXT NOT NULL, name TEXT NOT NULL, minutes INTEGER NOT NULL, "
            "PRIMARY KEY (slug))"
        )
        self._connection.commit()

    def add(self, slug: str, name: str, minutes: int) -> None:
        """Creates the title.

        Raises sqlite3.IntegrityError when the slug is already carried.
        """
        _ = self._connection.execute(
            "INSERT INTO titles (slug, name, minutes) VALUES (?, ?, ?)",
            (slug, name, minutes),
        )
        self._connection.commit()

    def carried(self, slug: str) -> tuple[str, int] | None:
        cursor = self._connection.execute(
            "SELECT name, minutes FROM titles WHERE slug = ?",
            (slug,),
        )
        row: tuple[str, int] | None = cursor.fetchone()
        return row

    def correct(self, slug: str, name: str, minutes: int) -> bool:
        cursor = self._connection.execute(
            "UPDATE titles SET name = ?, minutes = ? WHERE slug = ?",
            (name, minutes, slug),
        )
        self._connection.commit()
        return cursor.rowcount > 0

    def withdraw(self, slug: str) -> bool:
        cursor = self._connection.execute(
            "DELETE FROM titles WHERE slug = ?",
            (slug,),
        )
        self._connection.commit()
        return cursor.rowcount > 0

    def close(self) -> None:
        self._connection.close()
