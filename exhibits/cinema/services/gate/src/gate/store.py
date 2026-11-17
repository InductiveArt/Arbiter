import sqlite3
from pathlib import Path


class Store:
    """Members in one sqlite file: each handle with its name."""

    def __init__(self, path: Path) -> None:
        # The connection may be used from one thread at a time, not only its opener.
        self._connection = sqlite3.connect(path, check_same_thread=False)
        _ = self._connection.execute(
            "CREATE TABLE IF NOT EXISTS members ("
            "handle TEXT NOT NULL, name TEXT NOT NULL, "
            "PRIMARY KEY (handle))"
        )
        self._connection.commit()

    def enroll(self, handle: str, name: str) -> None:
        """Creates the member.

        Raises sqlite3.IntegrityError when the handle is already enrolled.
        """
        _ = self._connection.execute(
            "INSERT INTO members (handle, name) VALUES (?, ?)",
            (handle, name),
        )
        self._connection.commit()

    def named(self, handle: str) -> str | None:
        cursor = self._connection.execute(
            "SELECT name FROM members WHERE handle = ?",
            (handle,),
        )
        row: tuple[str] | None = cursor.fetchone()
        if row is None:
            return None
        return row[0]

    def rename(self, handle: str, name: str) -> bool:
        cursor = self._connection.execute(
            "UPDATE members SET name = ? WHERE handle = ?",
            (name, handle),
        )
        self._connection.commit()
        return cursor.rowcount > 0

    def strike(self, handle: str) -> bool:
        cursor = self._connection.execute(
            "DELETE FROM members WHERE handle = ?",
            (handle,),
        )
        self._connection.commit()
        return cursor.rowcount > 0

    def close(self) -> None:
        self._connection.close()
