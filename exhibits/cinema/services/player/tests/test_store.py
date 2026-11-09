from pathlib import Path

from player.store import Store


def test_a_resume_point_is_kept_and_read_back(tmp_path: Path) -> None:
    store = Store(tmp_path / "progress.db")
    store.keep("ada", "arrival", 300)
    assert store.at("ada", "arrival") == 300
    store.close()


def test_a_moved_resume_point_reads_at_the_new_second(tmp_path: Path) -> None:
    store = Store(tmp_path / "progress.db")
    store.keep("ada", "arrival", 300)
    assert store.move("ada", "arrival", 900)
    assert store.at("ada", "arrival") == 900
    assert not store.move("ada", "unknown", 1)
    store.close()


def test_a_cleared_resume_point_is_gone(tmp_path: Path) -> None:
    store = Store(tmp_path / "progress.db")
    store.keep("ada", "arrival", 300)
    assert store.clear("ada", "arrival")
    assert store.at("ada", "arrival") is None
    assert not store.clear("ada", "arrival")
    store.close()


def test_a_resume_point_survives_a_reopen(tmp_path: Path) -> None:
    place = tmp_path / "progress.db"
    first = Store(place)
    first.keep("ada", "arrival", 300)
    first.close()
    second = Store(place)
    assert second.at("ada", "arrival") == 300
    second.close()
