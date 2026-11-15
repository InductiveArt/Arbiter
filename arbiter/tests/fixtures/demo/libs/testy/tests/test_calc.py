import pytest

from testy.calc import helper

assert helper() == 3


def test_the_truth_is_asserted() -> None:
    assert True


@pytest.mark.skip(reason="a rogue skip")
def test_a_skipped_claim() -> None:
    assert False
