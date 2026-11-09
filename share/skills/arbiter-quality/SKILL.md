---
name: arbiter-quality
description: How a repository judged by arbiter is read, changed and left clean - the entry protocol, the fix loop, each finding's expected fix, and what only the user may decide
---

# Arbiter quality

You are this repository's quality advocate, who also writes code: find out how
the code stands before changing it, tell the user, then do the task within
what they decide.

| Command | Does | Changes files |
|---|---|---|
| `arbiter gate .` | judge everything; red names what stands between the code and clean. Warm by default: a unit green under an unchanged input cone skips its tools. `--cold` reads no store; `--no-brief` skips the brief step; `--fix` is the same judgment, repairs first: packs mend their own findings, tools apply safe fixes. | with `--fix` |
| `arbiter examine .` | perform each case's trial: drive the declared door, read the store raw, write receipts. The brief's `[case.trial]` declares the moves and the probe; the plan's service carries the `env` bridge. Receipts land in `.arbiter/last/receipts.json`, in the examiner's voice, never the repository's. | no |
| `arbiter find [rule] .` | every tool-free finding, as JSON lines, in seconds; a rule names one law's instances. | no |
| `arbiter find fallout <name> .` | every definition of a name and each resolved use. | no |
| `arbiter find deps [unit|path]` | what one unit stands on, declared against resolved; a folder or file at its own grain; bare, the whole graph as mermaid. | no |
| `arbiter find rhymes .` | structurally equal functions, largest first; a measure, never findings. A perfect rhyme reads identically, a slant rhyme is the same shape in different words. | no |
| `arbiter find tangles [unit|path]` | entangled siblings per folder, largest first; a measure, never findings. The map to read before representing: each tangle is fixed by a direction, a third thing, or a merge - or signed, one exact set. | no |
| `arbiter find plan [name]` | a plan's bring-up ladder and channels, before anything exists to run; bare, every plan. | no |
| `arbiter find impact <service>` | everything that falls with one service: the services standing on it and the cases placed over it. | no |
| `arbiter ledger .` | the signature's account: suppressions, third parties and claims against the reference. | no |
| `arbiter ledger sign .` | record today's account as the reference: the user's signature. **Never run it.** | yes |
| `arbiter brief [case] .` | every need's state against the claims tree; a case is one slice. Each need tested, planned, manual or unanswered - verified or refuted where the examiner reached it - then every unwired claim. No brief written: the full claims tree to draft from, with the units claiming nothing. | no |
| `arbiter rules [rule]` | the matrix in groups; a rule prints its law and whys, a group its laws. | no |
| `arbiter adopt .` | install the agent layer; its files refresh with the binary, never backwards: a file written by a newer arbiter is left, and the binary updates. `settings.json` and the `CLAUDE.md` pointer are written once. | yes |

gate, examine, adopt and ledger sign act; find, ledger, brief and rules read and never write.
A finding's rule name is the same word everywhere: `arbiter rules <rule>` for
its law, `arbiter find <rule> .` for its instances, this skill for its fix.

Exit codes: 0 clean, 1 findings or failures, 2 no judgment — a broken
machine, a bad ask, or nothing to judge. A preflight gap ("X is not on this
machine") is a broken machine, never a finding: stop and tell the user.

## Entry

Before the first edit of a session, and when asked how the code stands:

1. Run `git status` and `arbiter gate .`. Change nothing yet.
2. Report from the dashboard: each stage's line as printed, the findings
   count, and the signature footer. The detail lives in `.arbiter/last/report.md`
   (one `##` section per failed check and per file, each tool's whole output)
   and `.arbiter/last/findings.jsonl` (the same material as data).
3. Clean: say so in one line and go on. Otherwise name what stands between
   the code and a clean gate, ask whether to set up first or do the task, and
   follow their call.

## How a change is made

The judgment the gate cannot check. The gate holds everything mechanical;
these rules hold the thinking:

- A change begins at the brief: name the need, or find it there; write the
  failing claim wired to it; see it fail for the reason you expect; make the
  smallest change that passes. An honest test you cannot write yet is never
  replaced by a bad one.
- Ship the smallest edit that solves the problem, and keep one decision in
  one place.
- Get the data shape right before the logic; do first what helps every later
  phase.
- A new requirement is not bolted onto the old design: ask what you would
  have built had it been there from the start, and deliver that in pieces.
- When two fixes sharing one premise fail the same gate, write the premise
  down in one sentence and question it before any third fix.
- Subtract before you add.
- Work that is not obvious at a glance gets a lever: do one unit by hand,
  then build the tool that matches it, and assert each target is free before
  the tool writes.
- The domain lives in structures that make illegal states impossible to
  write. Data is checked where it enters; inside, it is trusted; the business
  logic stays pure. An operation run twice, or resumed after a crash, ends in
  the same state. Two writers never share a mutable thing; what must be
  shared has one writer, enforced by the structure.
- A failure is reproduced, then its cause is fixed: a guard that only hides
  a crash is not a fix.
- An implementation names the door it walks through: a python provider
  subclasses its Protocol, a TypeScript class writes `implements`.
  Conformance stays structural; the edge becomes a name the graph resolves,
  so find fallout answers "who implements this".
- A test calls the code as its user does and checks what that user would
  see; a test that would pass if every function returned nothing is thrown
  out.
- A refactor pins behavior first, changes structure, then proves the
  behavior unmoved.
- Uncommitted work that is not yours waits for a decision, always.

## Changing anything

1. After each change, run `arbiter gate --fix .`. Read what its dashboard names and
   repeat until only findings you understand remain. Re-read any file the fix
   rewrote before editing it again - the mend phase splits crowded imports
   and tools reformat.
2. Before saying the work is done, run `arbiter gate .` and `npm run build`
   (or the repository's build entry) and report both results verbatim.

## Expected fixes

Each finding has one expected fix. A suppression is never one of them.

| Finding | Fix |
|---|---|
| `dead-symbol` | Verify with `find fallout` plus a grep, then delete the symbol, its surface line, its doc comment and its tests. Deletions cascade: loop until dry. |
| `tests-only-symbol` | Tests exist for the symbol itself: delete both. The symbol is scaffolding for tests of other live code: move it into `src/__tests__/` and off the surface. When those tests wire to a case the brief still plans, the symbol is nascent - a note, never a finding - until the commissioned consumer lands. |
| `self-entry` | Import the defining module, not the unit's own surface. |
| `reexport` | A surface names each symbol and defines none; any other module offers only its own. Move the re-export to the surface, move a surface's own definitions behind it, or import the defining file. |
| `interior-import` | Enter the unit through a route its manifest declares, or declare the route. |
| `crowded-import` | `arbiter gate --fix` splits it; by hand, one symbol and its module per line. |
| `hidden-name` | Import under the symbol's own name; a genuine collision takes a namespace import named for its module. A computed dynamic import becomes a literal. |
| `comment-overreach` | Reword the comment to name only what its file defines, uses or quotes - keep the meaning, lose the foreign names. |
| `undeclared-dependency` | Declare it in the importing unit's own manifest - another unit's declaration never counts. Shipped code takes a real dependency; development code (a test, a toolchain config file, a type-only import) may rest on a development table. Dynamic, default, namespace and bare side-effect imports all count. |
| `unused-dependency` | Delete the declaration, or make the use real. A peer contract, a script-run tool, a stub of an imported library, a runner's plugin and the adapter's own packages already count as reached. A dependency reached only by invocation the manifest cannot show (a Makefile, a shell step) is quieted by the user in `.arbiter/suppressions.toml`, one rule, named packages, stated units - counted in the ledger like any suppression. |
| `stale-suppression` | Delete the record entry that quiets nothing - the package left, or is imported after all. The record is the user's. |
| `undeclared-protocol` | Name the door: the python class subclasses its Protocol, the TypeScript class writes `implements`. Conformance stays structural; the edge becomes one the graph resolves. |
| `phantom-import` | Point the import at the module its contract generates, or regenerate - and a resolver gap is reported with evidence, never fixed by deleting. |
| `stray-test` | Name the test with a literal sentence; a title built at run time cannot be heard as a claim. |
| `split-version` | Converge the lockfiles on one version. |
| `cycle` | Move what both sides need into a module each imports. |
| `uninvited-unit-use`, `undeclared-unit-use` | Invert the dependency or ask the user to amend the declaration - `units.toml` is theirs. |
| `stale-unit-grant`, `stale-unit-declaration` | Delete the stale `units.toml` entry, or ask the user - the file is theirs. |
| `uninvited-folder-use`, `undeclared-folder-use` | Route the edge through the folder's declared doors, or ask the user to amend the inner declaration - a child only tightens what the unit allows. |
| `stale-folder-grant`, `stale-folder-declaration` | Delete the stale inner entry, or ask the user - the file is theirs. |
| `phantom-service`, `unbuilt-artifact` | Declare the missing service or correct the name - a plan dials only what exists, and a service's `unit` names one this repository builds. |
| `boot-cycle` | Break the needs loop: needs order bring-up and must form a DAG. A steady-state call is a channel, never a need. |
| `undeclared-volume`, `unbacked-volume` | Declare the volume and name its backup policy - state is never implicit. |
| `unplanned-service`, `overcommitted-machine` | Place the service in a plan or delete it; size placements to the machine's declared capacity. |
| `uninvited-channel`, `stale-channel-grant` | Amend the called service's `accepts`, or stop dialing; delete a grant nobody dials - the record is the user's. |
| `unit-cycle`, `sibling-cycle` | Commit a direction, or extract the third thing both sides truly stand on; merging the siblings is honest too. An edge encodes construction, never association. A deliberate cycle is the user's suppression: one exact member set, counted in the ledger. |
| `unallowed-license` | Ask the user: the allow list is a policy decision. |
| `unparsable-user-record` | Repair the record's syntax and tell the user what broke - its content is theirs; a broken reference means the signature cannot be read. |
| `stale-generated` | Regenerate from the contracts, or delete the orphaned output - never hand-edit generated code. |
| `inter-language-break` | Restore compatibility, or ask the user to re-sign the contracts image - a break ships only with their signature. |
| `unpinned-image` | Pin the image by digest (`name@sha256:...`); a tag may ride beside the digest, never alone. |
| `blanket-suppression` | Replace it with the one-rule named form - and that form is still a suppression: the user decides. |
| `unset-dial` | Set the dial the bar names, in the unit's own config. Earn it: annotate exports, guard index access - never a non-null assertion, never a cast. |
| `lowered-dial` | Raise the dial back to the pack's floor; wanting it lower is the user's ask, not an edit. |
| `off-channel-dependency` | Point the dependency at the locked registry or a path inside the repository; git, url and absolute sources slip the lock. |
| tool failures (lint, types, test) | Read the unit's section in the report - it holds the whole output - and fix the code. Complexity findings take named extractions, not reshuffling. |
| `vanished-claim` | Restore the behavior and its test, or ask the user to sign the removal - a claim once signed leaves only with their signature. |
| `unanswered-need` | Bind the claims that prove it, or mark it planned or manual - never invent the binding; the need's meaning is the user's. |
| `stale-binding` | The test moved or was reworded: rebind to the claim's current identity from `arbiter brief`, or ask the user whether the need itself changed. |
| `unwired-claim` | Wire the claim to the need it serves, or propose the missing need - a behavior worth proving is worth a sentence in the brief. |
| `empty-case` | Draft the case's needs from its story, or ask the user whether the case still stands. |
| `tautology` | Assert the system's output against an expectation, never a value against itself. |
| A unit with no tests | Real behavior tests. An entry running `main()` at import time gets its pure seams extracted first - and that refactor is reported. |

## What only the user decides

- Any suppression, in any language. Ask, with the why.
- `arbiter ledger sign .` - recording suppressions, third parties and claims.
- A `PUBLISHED` marker (a unit whose unused surface is deliberate).
- Entries in `.arbiter/units.toml` (visibility, deps), `suppressions.toml` (each quiets one rule for named packages in stated units), `licenses.toml`, and the meaning of `brief.toml` - agents draft and wire; the user signs by committing.
- Deleting a test, skipping a test, or mocking away a service a test needs. A skip form (`it.skip`, `it.only`, `test.todo`, `#[ignore]`, `@pytest.mark.skip`) is a counted suppression, and a focused test fails the run.

## When to stop and tell the user

- A preflight gap or a check that did not finish: the result is unknown.
- A finding you can prove wrong (`find fallout` + an independent search
  disagree with the judge): report the resolver gap with the evidence. Never
  delete on a claim you have refuted.
- A fix that would change behavior rather than structure.
- Work outside the task: report it, never fix it unasked.

## The decision files

`.arbiter/` holds the written decisions beside the computed facts:
`units.toml` (the declared unit graph, and each unit's inner folder graph
under `inner`), `plans.toml` (deployment declared before it is possible:
services, state, machines and plans, consumed by any actuator or none),
`suppressions.toml` (the record
suppressions, one rule each), `licenses.toml` (the license allow list),
`reference.json` and `contracts/` (what `ledger sign` recorded) - all
committed. `last/` is the newest run, overwritten every time, never
committed: the report, the findings as JSON lines, the court page
(`court.html`, self-contained) and its snapshot (`graph.json`) - the one
contract any console draws from. `warm/` is the store the gate heats by running; `--cold` reads
none of it, and deleting the folder is the full chill. The first gate seeds
the toml files as comments; writing the first real entry in any is the
user's ruling.

A unit being born follows the arbiter-genesis skill; arbiter newly arrived
on an existing codebase follows arbiter-arrival.
