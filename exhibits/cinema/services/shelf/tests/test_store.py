from pathlib import Path

from shelf.store import Store


def test_a_shelved_title_reads_back(tmp_path: Path) -> None:
    store = Store(tmp_path / "shelf.db")
    assert store.shelve("ada", "arrival")
    assert not store.shelve("ada", "arrival")
    assert store.titles("ada") == ["arrival"]
    store.close()


def test_an_unshelved_title_leaves_the_shelf(tmp_path: Path) -> None:
    store = Store(tmp_path / "shelf.db")
    assert store.shelve("ada", "arrival")
    assert store.unshelve("ada", "arrival")
    assert store.titles("ada") == []
    assert not store.unshelve("ada", "arrival")
    store.close()
