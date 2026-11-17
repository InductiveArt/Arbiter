from pathlib import Path

from reader.store import Store


def test_a_bookmark_is_kept_and_read_back(tmp_path: Path) -> None:
    store = Store(tmp_path / "marks.db")
    store.mark("ada", "sicp", 12)
    assert store.where("ada", "sicp") == 12
    store.close()


def test_a_moved_bookmark_reads_at_its_new_page(tmp_path: Path) -> None:
    store = Store(tmp_path / "marks.db")
    store.mark("ada", "sicp", 12)
    assert store.move("ada", "sicp", 40)
    assert store.where("ada", "sicp") == 40
    assert not store.move("ada", "unknown", 1)
    store.close()


def test_a_forgotten_bookmark_is_gone(tmp_path: Path) -> None:
    store = Store(tmp_path / "marks.db")
    store.mark("ada", "sicp", 12)
    assert store.forget("ada", "sicp")
    assert store.where("ada", "sicp") is None
    assert not store.forget("ada", "sicp")
    store.close()


def test_a_bookmark_survives_a_reopen(tmp_path: Path) -> None:
    place = tmp_path / "marks.db"
    first = Store(place)
    first.mark("ada", "sicp", 12)
    first.close()
    second = Store(place)
    assert second.where("ada", "sicp") == 12
    second.close()
