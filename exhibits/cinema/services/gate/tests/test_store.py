from pathlib import Path

from gate.store import Store


def test_a_member_is_enrolled_and_read_back(tmp_path: Path) -> None:
    store = Store(tmp_path / "members.db")
    store.enroll("ada", "Ada Lovelace")
    assert store.named("ada") == "Ada Lovelace"
    store.close()


def test_a_renamed_member_reads_at_the_new_name(tmp_path: Path) -> None:
    store = Store(tmp_path / "members.db")
    store.enroll("ada", "Ada Lovelace")
    assert store.rename("ada", "Ada King")
    assert store.named("ada") == "Ada King"
    assert not store.rename("unknown", "Nobody")
    store.close()


def test_a_struck_member_is_gone(tmp_path: Path) -> None:
    store = Store(tmp_path / "members.db")
    store.enroll("ada", "Ada Lovelace")
    assert store.strike("ada")
    assert store.named("ada") is None
    assert not store.strike("ada")
    store.close()


def test_the_member_roll_survives_a_reopen(tmp_path: Path) -> None:
    place = tmp_path / "members.db"
    first = Store(place)
    first.enroll("ada", "Ada Lovelace")
    first.close()
    second = Store(place)
    assert second.named("ada") == "Ada Lovelace"
    second.close()
