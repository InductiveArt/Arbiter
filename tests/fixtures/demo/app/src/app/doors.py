from typing import Protocol


class Port(Protocol):
    def send(self, note: str) -> str: ...


class Satellite:
    def send(self, note: str) -> str:
        return note


def beamed(note: str, port: Port) -> str:
    return port.send(note)


print(beamed("hi", Satellite()))
