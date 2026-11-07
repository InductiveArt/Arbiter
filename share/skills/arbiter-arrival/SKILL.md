---
name: arbiter-arrival
description: When arbiter arrives on an existing codebase - reading the first red gate as a map, the campaign order toward the bar, and how the work continues while the debt shrinks
---

# Arbiter arrives on living code

The first gate on an old codebase is red. Red is a map, not a wall: the
work continues from day one, everything touched meets the bar, and the
campaigns land in the user's order.

## Day one

1. `arbiter adopt .` then `arbiter gate .`. Commit the agent layer and the
   seeded `.arbiter/` files first, so every later session shares one gate.
2. A preflight gap ("X is not on this machine") is a broken machine, never
   a finding: the machine is fixed before anything is judged.
3. Report the dashboard whole: each failing check, the finding counts by
   rule, the suppression count, the brief line. The detail lives in
   `.arbiter/last/report.md`, one section per failure. Then ask the user
   which campaign to open, and do the asked task meanwhile.

## The campaigns, in order

Each is its own commit, opened at the user's word; none blocks the task at
hand.

1. **Ecosystem dials.** Each unit's declared environment meets the bar; the gate
   names every unset or lowered option, unit by unit. Raise the
   configuration, never lower the bar.
2. **The mechanical pass.** One `arbiter gate --fix .`: packs mend their
   own findings and tools apply safe fixes. Commit that rewrite alone, so
   every later fix touches only what a task changes.
3. **Tool failures.** Lint, types, tests - one stage at a time. Arbiter
   keeps no baseline of old errors: the path is fixing them, and what the
   user chooses to silence instead is their suppression, counted by the
   ledger from then on.
4. **Graph findings.** winnow and integrity name the oldest debt: dead
   symbols, cycles, re-exports, hidden names, undeclared and unused
   dependencies. One rule at a time; a deletion follows the dead-symbol
   fix - find fallout, an independent grep, then delete until dry.
5. **The brief.** The existing tests are already claims; `arbiter brief .`
   prints the whole tree to draft from. Draft cases from the frames and
   wire every claim; the user signs by committing. From the first written
   case the gate holds every need answered and every claim wired.
6. **The signature.** The user runs `arbiter ledger sign .` and today's
   suppressions, dependencies and claims become the reference: the gate
   then names only what moved. That is the ratchet - debt frozen, nothing
   new unsigned.

## While the campaigns wait

- Do the asked task inside the law: a file you touch leaves clean even
  when its neighbors are not.
- Never silence a finding to get green, and never reword code so a check
  stops matching it. Both are the user's decisions.
- A finding outside the task is reported, never fixed unasked.
- When the task collides with the debt - its unit fails its dials, its test
  needs a service - say so and ask whether to set up first or do the task.
