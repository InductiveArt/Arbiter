---
name: arbiter-method
description: Use before any substantive problem or improvement - carve the axes, read coordinates not anomalies, move the generator never the output, and execute the improvement whole.
---

# Find the generators

A method in two movements that are one principle. First: understand a
problem by finding the independent axes it lives on. Second: never fix
an output; find what generates it and move that, whole. Continuous
improvement, done right, is nothing but the second movement applied
every day.

## The basis

To understand anything, first find the axes on which it stands:
independent dimensions that together describe everything observed. Seen
through the right axes, even actions that look incoherent become exactly
what they are: positions in the space.

Religion is the proving example. It is not one block but three axes:
croire (belief), pratique (practice) and appartenance (belonging). A
person who practices without believing is not a contradiction; they are
a coordinate.

Core axes don't need to be orthogonal, but they do need to be
independent. "Orthogonal" bundles three ideas that come apart:

1. Independence. You can move one axis without the definition of another
   moving with it. Croire, pratique and appartenance pass this test: you
   can believe without practicing, or belong without believing. That is
   why apparently incoherent behavior falls out of the model. Incoherence
   is an unusual coordinate, not an anomaly.
2. Uncorrelated in practice. In real populations those three axes are
   strongly correlated: most people who practice also believe. That does
   not make them less valid as axes. Correlation is a fact about the
   data, not about the basis.
3. Unconstrained. You can push every knob to any value at once. This
   almost never holds, because budget, physics and time shrink the space.

A good basis needs (1). It can do without (2) and (3).

## Coordinates, not anomalies

Once the basis is found, the strange cases stop being exceptions to
argue about and become positions to read. A suppression is a violation
in facts, a law held, an exception in authority. Dead code commissioned
by a written plan is nascent, not dead. The move is always the same:
when something looks incoherent, do not patch it and do not excuse it;
ask which axes it sits on, and read its coordinate.

## Generator space

Compromise happens in output space; composition happens in generator
space. The engineer's job is to find the generators.

The clean example is color. A photograph is the same pixels in RGB and
in HSL, but "make it warmer" is a fight in RGB: three numbers per pixel,
each edit disturbing the others, every adjustment a compromise. In HSL
it is one knob, because hue is a generator of the image and red-green-blue
are its outputs. Nothing about the picture changed; the basis did. That
is the tell to memorize: when a fix touches many places at once and each
touch fights the last, you are editing outputs. Stop, and find the knob.

In software the outputs are the defects, the slow pages, the flaky
tests, the inconsistent names. The generators are the process, the law,
the schema, the one function everything flows through. A bug fixed where
it surfaced is an output edit: the same generator will print the same
bug somewhere else next week. The fix that counts changes what generated
it, and makes the whole class impossible.

## The Toyota line

Continuous improvement is generator-space composition applied over time.
That is the bridge between the two movements, and Toyota built the
discipline: the Toyota Production System, from which lean and the whole
of DevOps descend. Its practices are each a generator-space rule wearing
work clothes:

- **Genchi genbutsu - go and see.** Facts come from the floor, never
  from memory or a report about a report. Run the thing; read what it
  printed today. How anything stands is never remembered.
- **Jidoka - stop the line.** A defect stops work at the defect. The
  alternative, a rework station at the end of the line, is compromise in
  output space made into furniture: the generator keeps printing defects
  and a team keeps polishing them. A red gate that gets worked around is
  a rework station.
- **Five whys.** The walk from output space back to generator space. Ask
  why until the answer is a process or a law, not an event or a person.
  Stopping at three whys leaves you holding an output.
- **Kaizen.** Small improvements, continuously, by whoever is closest to
  the work - never a big-bang program, never a committee. The size of
  the step is small; the execution of the step is total.
- **Standardize.** An improvement exists when the generator changed and
  the change became the new standard - written where the work happens,
  held by something that cannot forget. Until then it is a suggestion in
  a box.

## Done means the generator moved

No half measures. Every improvement, however small, executes whole:

1. The defect stopped the line. Nothing shipped around it, nothing
   silenced it.
2. The whys reached a generator - a law, an engine, a record, a habit -
   not a symptom or a culprit.
3. The generator changed whole. A real addition respects its scope; a
   feature done as a patch is an output edit wearing a feature's name,
   and it is refused.
4. The new standard landed where work happens: a rule the gate holds, a
   test that claims it, a record the machine reads. If it can regress
   silently, it did not land.
5. A run proved it, the same day. Then the next small step.

## Applying it

Before building anything substantive: name the axes of the problem and
test them for independence. Place the incoherent cases as coordinates
before touching them. Ask what generates the outputs you dislike, and
change that - whole, at its own scope, with its staleness guarded. Land
the standard so the improvement cannot silently unhappen. Prove it with
a run. When a change starts to feel like a patch, that feeling is the
finding: return to the generator.
