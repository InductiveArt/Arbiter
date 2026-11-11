# Arbiter processes: why what we do exists

Arbiter is made of processes: rules that check that something actually
happened in a process step, where that step may itself be a substep of a
larger process. This document sets out the rationale for what we do. That
means justifying every step, including the gate steps; explaining why each
thing is named as it is; and stating exactly what each thing provides.

To do that, we need the axes on which our work stands: independent
dimensions that together describe everything we do. Seen through those three axes,
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
coordinate, not an anomaly. Each law detains its power from one of two
spaces: the code space — the facts alone, the bar from day one — or the
user space, a record whose law waits until the record is written; the
drill names each rule's power.

**Authority.** Who may decide: packs state laws, agents work inside them,
the user signs what crosses them — suppressions, boundaries, the brief's
meaning, a claim's removal. Authority moves alone: same facts, same law,
and the signature turns a counted suppression from debt into reference.

**Evidence.** Every step proves it happened: the dashboard's line per
step, the findings, `.arbiter/last/` - the report, the court page and
its snapshot - the reference, the stamps adopted files carry. Evidence moves alone: the same judgment can leave a richer
or leaner trace.

**Economy.** Judge only what moved: the warm cone, the pinned versions,
the generation digests, the ratchet against the reference. Economy moves
alone: a cold run changes cost and redefines nothing.

**Teaching.** The word-surface: one name at every door, the offense
register, the drills, the skills. Teaching moves alone — proven when every
law-register handle was renamed in one release and no behavior moved.

## Coordinates, not anomalies

- A suppression, on a line or in the record: violation in facts, law
  held, exception in authority - and counted, either way.
- Nascent code: dead by law, commissioned by authority through a planned
  need.
- A `PUBLISHED` unit: unused in facts, kept by the user's marker.
- A planned need: law without facts yet, signed by being written.
- An open matrix cell: a law waiting for its facts-engine.

## The planes

The system splits into planes the way a network does, and the names are
kept deliberately close to telecom's, because this existed before us:

- **Intent**: the records - the unit graph, the suppressions, the brief,
  the plans. Written by the user, drafted by agents and the console.
- **Facts**: what machines materialize and arbiter observes - manifests,
  locks, resolved edges.
- **Law and assurance**: arbiter - judging intent against facts, never
  actuating. The console is this plane's window, on the desired side.
- **Actuation**: whatever consumes the records to make anything real - a
  reconciler, a bundler, plain docker, or nobody. Deliberately an
  interface, not an implementation; the records assume no consumer.
- **Data**: the running system itself, speaking its contracts.

The iron rules: facts flow up, intent flows down, and a signature gates
every crossing. Arbiter never actuates; an actuator never judges.

## The ontology

Everything the records and the facts describe is one graph, stratified.
The strata, top down: intent - case, need; runtime - plan, machine,
service, channel, volume, door; structure - unit, folder, file; code -
symbol, import; evidence - claim, trial, receipt, finding. Horizontal
edges live inside one stratum: a service needs or dials a service, a unit
imports a unit. One spine of vertical bindings crosses the strata: a case
sits on a unit and serves a need, a trial proves needs, a service names
its unit, a claim answers a need.
The snapshot is this graph serialized; the court is a lens over it,
never a second model, and any future consumer - a reconciler watching
the live system, an alerting plane - reads the same graph rather than
rediscovering it.

Every assertion in the graph carries one of three powers, and each power
has its guard:

**Derived.** Computed from the tree: imports resolved, units carved,
claims heard, cones and impact walked. Cannot lie, and costs nobody a
line of annotation.

**Declared.** Signed into a record: the unit graph, the plans, the brief
with its trials. Never computable - intent does not fall out
of code, and reading the tree back to itself and calling it purpose is
the mirror trap. Kept true by laws: phantom-service, misplaced-case,
orphan-case, unmandated-trial, stale-mandate, vanished-claim. Drift in
a declaration is a finding, so a declaration
cannot quietly rot - which is what separates this graph from every
architecture drawing maintained beside its system.

**Observed.** Witnessed by the examiner at trial: the world stood up
from the records alone, driven through its declared door, read raw at
the store over the examiner's own connection. This is where derivation
grows - not by guessing harder at static code, but by adding witnesses:
a declared impact held against a killed service.

The annotation budget is closed. What is, is derived; what should be, is
declared; what connects them is bound - and a binding is admitted only
when it is checkable against facts at one end, counted in the ledger,
and one line to write. Everything else the machine says itself.

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
An edge is sanctioned by both ends: the user's deps consent outward,
the owner's visibility inward, and an absent declaration binds nothing.
Declarations attach at two grains: the unit, judged on the quotient
graph, and the folder within it - a file is a leaf of that tree - judged
over every declared node an end sits under, so a child only tightens
what the unit allows. A node governs its border, never its interior.
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

**Plans.** Provides the declared deployment: services and the state they
keep, the machines expected, the plans placing them. Bring-up needs are
construction and must form an order; channels are association, carried
by the called service's consent. Judged like any record, consumed by
any actuator or none. Laws over intent. Evidence: the findings, and
`find plan` as the storyboard.

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
