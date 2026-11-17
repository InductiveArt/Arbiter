from pathlib import Path

from tickets.store import Store


def test_a_rental_is_opened_and_read_back(tmp_path: Path) -> None:
    store = Store(tmp_path / "tickets.db")
    store.rent("ada", "arrival", 2)
    assert store.standing("ada", "arrival") == 2
    store.close()


def test_an_extended_rental_reads_at_the_new_nights(tmp_path: Path) -> None:
    store = Store(tmp_path / "tickets.db")
    store.rent("ada", "arrival", 2)
    assert store.extend("ada", "arrival", 5)
    assert store.standing("ada", "arrival") == 5
    assert not store.extend("ada", "unknown", 1)
    store.close()


def test_a_cancelled_rental_is_gone(tmp_path: Path) -> None:
    store = Store(tmp_path / "tickets.db")
    store.rent("ada", "arrival", 2)
    assert store.cancel("ada", "arrival")
    assert store.standing("ada", "arrival") is None
    assert not store.cancel("ada", "arrival")
    store.close()


def test_rentals_survive_a_reopen(tmp_path: Path) -> None:
    place = tmp_path / "tickets.db"
    first = Store(place)
    first.rent("ada", "arrival", 2)
    first.close()
    second = Store(place)
    assert second.standing("ada", "arrival") == 2
    second.close()
