# Arbiter processes: why what we do exists

Arbiter is made of processes: rules that check that something actually
happened in a process step, where that step may itself be a substep of a
larger process. This document sets out the rationale for what we do. That
means justifying every step, including the gate steps; explaining why each
thing is named as it is; and stating exactly what each thing provides.

To do that, we need the axes on which our work stands: independent
dimensions that together describe everything we do. Religion is a useful
example. It is not one block but three axes: croire (belief), pratique
(practice) and appartenance (belonging). Seen through those three axes,
even actions that look incoherent become exactly what they are: positions
in the space.

Compromise happens in output space; composition happens in generator
space. The engineer's job is to find the generators. Core axes don't need
to be orthogonal, but they do need to be independent.

"Orthogonal" bundles three ideas that come apart:

1. Independence. You can move one axis without the definition of another
   moving with it. Croire, pratique and appartenance pass this test: you
   can believe without practicing, or belong without believing. That is
   why apparently incoherent behavior falls out of the model. Incoherence
   is an unusual coordinate, not an anomaly.
2. Uncorrelated in practice. In real populations those three axes are
   strongly correlated: most people who practice also believe. That does
   not make them less valid as axes. Correlation is a fact about the data,
   not about the basis.
3. Unconstrained. You can push every knob to any value at once. This
   almost never holds, because budget, physics and time shrink the space.

A good basis needs (1). It can do without (2) and (3).

## The six axes

**Facts.** What the machine observes, judging nothing: units carved from
manifests, imports resolved to edges, claims heard from tests, suppressions
counted, dials read, versions locked. Facts move alone: `find deps` is
facts with no law applied. The resolution engine lives here — and a gap in
the facts is proven and reported, never accused.

**Laws.** What must hold over the facts: the matrix, the brief's needs,
the license allowlist, the declared boundaries. Laws move alone: a
language column can hold every import law as `open` — laws stated, no
facts-engine under them yet. An open cell is law-without-facts: a
coordinate, not an anomaly.

**Authority.** Who may decide: packs state laws, agents work inside them,
the user signs what crosses them — suppressions, boundaries, the brief's
meaning, a claim's removal. Authority moves alone: same facts, same law,
and the signature turns a counted suppression from debt into reference.

**Evidence.** Every step proves it happened: the dashboard's line per
step, the findings, `.arbiter/last/`, the reference, the stamps adopted
files carry. Evidence moves alone: the same judgment can leave a richer
or leaner trace.

**Economy.** Judge only what moved: the warm cone, the pinned versions,
the generation digests, the ratchet against the reference. Economy moves
alone: a cold run changes cost and redefines nothing.

**Teaching.** The word-surface: one name at every door, the offense
register, the drills, the skills. Teaching moves alone — proven when every
law-register handle was renamed in one release and no behavior moved.

## Coordinates, not anomalies

- A suppression: violation in facts, law held, exception in authority.
- Nascent code: dead by law, commissioned by authority through a planned
  need.
- A `PUBLISHED` unit: unused in facts, kept by the user's marker.
- A planned need: law without facts yet, signed by being written.
- An open matrix cell: a law waiting for its facts-engine.

## The processes

The gate is the master process. Each step below is one of its substeps,
in running order. For each: what it provides, why it exists, and its
evidence — the witness that the step actually happened.

**Scan.** Provides the units: manifests carve the repository, and every
file belongs to the deepest unit claiming it. Containers bound without
being judged; tests, fixtures, generated files and git-ignored folders
are told apart from code, because each is read differently ever after.
Facts. Evidence: the dashboard's `N units, N files`.

**Resolve.** Provides the edges: every import reaches the file its module
path names, through routes, barrels and chains; a literal run-time import
keeps its surface; generated code answers for the contract it was
generated from. This is the engine every import law rides — it judges
nothing, and that is why it is not a rule: a law fires, an engine makes
the facts laws fire on. When the engine fails on a real missing module,
the type-checker fires in the tools step; when it fails from its own
blindness, nobody may fire — the gap is proven with `find fallout` and an
independent search, then reported. Facts. Evidence: the edges every
finding cites, file and line.

**Judge.** Provides the findings: the graph laws held over the facts —
symbols with no user, imports that hide or crowd or re-export, units
crossing boundaries, doors left unnamed, comments reaching too far.
Every finding names the law it breaks, and the same word answers at
every door: `rules` for the law, `find` for the instances, the skill for
the fix. Laws. Evidence: the findings themselves, named and placed.

**Dials.** Provides the bar: each unit's declared configuration — the
ecosystem's dials — held to the pack's floor. A setting may add, never
lower, because the cheapest green is a loosened judge. The same files
declare where dependencies come from, so a source slipping the locked
registry is named here too. Laws over facts, bounded by authority.
Evidence: a finding per unset dial, lowered dial or off-channel source,
at the unit's config file — read fresh every run, warm or not, because a
finding never rests on the store. TypeScript's dials resolve through the
pinned tsc, which is why these laws answer at the gate, not at find.

**Tools.** Provides the verdicts code cannot self-report: lint, types,
tests. The pack pins the versions — the pins are not checked, they are
constitution: a unit never chooses its own judge. Warnings fail, never
float. Laws, priced by economy. Evidence: one outcome per check per
unit, the whole output in the report.

**Contracts.** Provides the inter-language check, twice: generated code
matches what its contracts generate (stale-generated), and the contracts
themselves break their signed image only with the user's signature
(inter-language-break). Digest-keyed so unmoved contracts cost nothing.
Laws, priced by economy. Evidence: outcomes per unit.

**Claims.** Provides the heard tree: every test sentence, under its
frames, by unit. The brief binds needs to claims both ways — every need
answered or saying why not, every claim answering a need. The needs'
meaning is the user's; a case still planned commissions its tests, which
is why nascent code is a note and not a finding. Laws bound to
authority. Evidence: the claims count, the brief states, each unwired
claim with its paste-ready identity.

**Ledger.** Provides the account: suppressions, third parties and claims,
counted against what the signature recorded. Authority made visible.
Evidence: the signature footer, and `arbiter ledger` as the drill.

**Sign.** Provides the reference: the user's act, never an agent's. From
then on the gate names only what moved — the ratchet. Authority.
Evidence: `reference.json` and the contracts images.

**Adopt.** Provides the taught layer: the rules file, the skills, the
stop hook, each stamped by the arbiter that wrote it and never written
backwards. Teaching. Evidence: the stamps.

## The constitution

What no step may do, whatever it would buy:

- A setting may add, never lower.
- A resolver gap is proven and reported, never accused.
- A law fires only where it cannot be wrong; the tail is covered by
  doctrine, not by guessing. Under-enforcement is deliberate.
- The pins are constitution, not checks.
- A suppression names one rule, covers one line, and is the user's.
- One name at every door: a rule's name is what `find` finds; an act is
  a verb, a report is a noun.
- The matrix holds laws only. What describes the machine lives here.
- Agents work inside the judgment; the user signs what crosses it.
