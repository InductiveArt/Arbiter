interface Beam {
  send(note: string): string;
}

class Dish {
  send(note: string): string {
    return note;
  }
}

function beamed(note: string, beam: Beam): string {
  return beam.send(note);
}

beamed("hi", new Dish());
