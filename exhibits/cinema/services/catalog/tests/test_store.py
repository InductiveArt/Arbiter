from pathlib import Path

from catalog.store import Store


def test_a_title_is_added_and_read_back(tmp_path: Path) -> None:
    store = Store(tmp_path / "titles.db")
    store.add("arrival", "Arrival", 116)
    assert store.carried("arrival") == ("Arrival", 116)
    store.close()


def test_a_corrected_title_reads_at_the_new_minutes(tmp_path: Path) -> None:
    store = Store(tmp_path / "titles.db")
    store.add("arrival", "Arrival", 116)
    assert store.correct("arrival", "Arrival", 118)
    assert store.carried("arrival") == ("Arrival", 118)
    assert not store.correct("unknown", "Nothing", 1)
    store.close()


def test_a_withdrawn_title_is_gone(tmp_path: Path) -> None:
    store = Store(tmp_path / "titles.db")
    store.add("arrival", "Arrival", 116)
    assert store.withdraw("arrival")
    assert store.carried("arrival") is None
    assert not store.withdraw("arrival")
    store.close()


def test_the_catalog_survives_a_reopen(tmp_path: Path) -> None:
    place = tmp_path / "titles.db"
    first = Store(place)
    first.add("arrival", "Arrival", 116)
    first.close()
    second = Store(place)
    assert second.carried("arrival") == ("Arrival", 116)
    second.close()
