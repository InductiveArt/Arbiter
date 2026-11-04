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
| `arbiter gate .` | Judges everything: each unit's pinned tools and the resolved graph. Warm by default: a unit green under an unchanged input cone skips its tools, and the dashboard counts it. `--cold` reads no store. | no |
| `arbiter gate --fix .` | The same judgment, repairs first: packs mend their own findings, tools apply safe fixes. | yes |
| `arbiter findings .` | The graph plane alone as JSON lines, in seconds. | no |
| `arbiter blast <name> .` | Each definition of a name and every resolved use of that one. | no |
| `arbiter ledger .` | Every suppression, counted against the accepted reference. | no |
| `arbiter rules` | The rule-by-language matrix - held, never with its why, or open - then each rule's and each core judgment's one-line law. | no |
| `arbiter proof .` | Every behavior the tests claim, as sentences in a tree by unit; the footer names units proving nothing. The capability map: read it before expanding code to learn what is and what is not. | no |
| `arbiter rhyme .` | Structurally equivalent functions, grouped and largest first: a perfect rhyme reads identically, a slant rhyme is the same shape in different words. A measure to read and judge - never findings, never a gate stage. | no |
| `arbiter accept .` | Records the suppression and dependency references. **The user's signature - never run it.** | yes |

Exit codes: 0 clean, 1 findings or failures, 2 nothing to judge or a broken
machine. A preflight gap ("X is not on this machine") is a broken machine,
never a finding: stop and tell the user.

## Entry

Before the first edit of a session, and when asked how the code stands:

1. Run `git status` and `arbiter gate .`. Change nothing yet.
2. Report from the dashboard: each stage's line as printed, the findings
   count, and the ledger footer. The detail lives in `.arbiter/last/report.md`
   (one `##` section per failed check and per file, each tool's whole output)
   and `.arbiter/last/findings.jsonl` (the same material as data).
3. Clean: say so in one line and go on. Otherwise name what stands between
   the code and a clean gate, ask whether to set up first or do the task, and
   follow their call.

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
| `winnow` (no user anywhere) | Verify with `blast` plus a grep, then delete the symbol, its surface line, its doc comment and its tests. Deletions cascade: loop until dry. |
| `winnow` (no user but tests) | Tests exist for the symbol itself: delete both. The symbol is scaffolding for tests of other live code: move it into `src/__tests__/` and off the surface. |
| `self-entry` | Import the defining module, not the unit's own surface. |
| `no-reexport` | A surface names each symbol and defines none; any other module offers only its own. Move the re-export to the surface, move a surface's own definitions behind it, or import the defining file. |
| `interior-import` | Enter the unit through a route its manifest declares, or declare the route. |
| `one-import-per-line` | `arbiter gate --fix` splits it; by hand, one symbol and its module per line. |
| `no-hidden-names` | Import under the symbol's own name; a genuine collision takes a namespace import named for its module. A computed dynamic import becomes a literal. |
| `no-comment-overreach` | Reword the comment to name only what its file defines, uses or quotes - keep the meaning, lose the foreign names. |
| `undeclared-dependency` | Declare it in the importing unit's own manifest - another unit's declaration never counts. Shipped code takes a real dependency; development code (a test, a toolchain config file, a type-only import) may rest on a development table. Dynamic, default, namespace and bare side-effect imports all count. |
| `one-version` | Converge the lockfiles on one version. |
| `cycle` | Move what both sides need into a module each imports. |
| boundaries (`units.toml`) | Invert the dependency or ask the user to amend the declaration - the file is theirs. |
| `licenses` | Ask the user: the allow list is a policy decision. |
| wiring (option below the bar) | Earn the option: annotate exports, guard index access. Never a non-null assertion, never a cast, never lowering the config. |
| tool failures (lint, types, test) | Read the unit's section in the report - it holds the whole output - and fix the code. Complexity findings take named extractions, not reshuffling. |
| A unit with no tests | Real behavior tests. An entry running `main()` at import time gets its pure seams extracted first - and that refactor is reported. |

## What only the user decides

- Any suppression, in any language. Ask, with the why.
- `arbiter accept .` - recording suppressions and dependency references.
- A `PUBLISHED` marker (a unit whose unused surface is deliberate).
- Entries in `.arbiter/units.toml` (visibility, deps) and `licenses.toml`.
- Deleting a test, skipping a test, or mocking away a service a test needs.

## When to stop and tell the user

- A preflight gap or a check that did not finish: the result is unknown.
- A finding you can prove wrong (`blast` + independent search disagree with
  the judge): report the resolver gap with the evidence. Never delete on a
  claim you have refuted.
- A fix that would change behavior rather than structure.
- Work outside the task: report it, never fix it unasked.

## The decision files

`.arbiter/` holds the written decisions beside the computed facts:
`units.toml` (the declared unit graph), `licenses.toml` (the license allow
list), `reference.json` and `contracts/` (what `accept` recorded) - all
committed. `last/` is the newest run, overwritten every time, never
committed. The first gate seeds the two toml files as comments; writing the
first real entry in either is the user's ruling.
