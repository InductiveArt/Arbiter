---
name: arbiter-genesis
description: How a new unit is born in a repository arbiter judges - the birth order from brief to clean gate, each language's minimal anatomy, and the proto contract dance
---

# A unit is born

Birth order, whatever the language. The gate teaches as you go: run
`arbiter gate .` after each step and fix what it names before the next.

1. The brief first: add the case, or the need under an existing case, that
   commissions the unit, as `planned = true`. A unit is born commissioned,
   never orphaned.
2. The manifest: the smallest shape below. The next gate judges wiring and
   names each option the bar demands - write what it asks, never less.
3. Join the repository: workspace membership where the ecosystem keeps one
   (`pnpm-workspace.yaml`, `[workspace]` members); a `visibility` entry in
   `.arbiter/units.toml` when the unit is not for everyone; the allowlist
   in `licenses.toml` when a new third party arrives. Both files are the
   user's: draft, and they sign by committing.
4. The first claim: write the failing test wired to the planned need, watch
   it fail for the reason you expect, make it pass, then trade `planned`
   for the claim bindings.
5. `arbiter gate .` clean, then commit manifest, code, test and brief edit
   as one birth.

## Anatomy

The minimal shape each pack expects. A dependency goes in the unit's own
manifest, always; the lockfile is the ecosystem's.

- **typescript** - `package.json` naming the unit and its exports route,
  `tsconfig.json` extending the repository's base, code in `src/`, tests in
  `__tests__/` folders or `*.test.ts` names. The surface is the file the
  exports route names: it names each symbol it offers and defines none.
- **python** - `pyproject.toml` with `[project]` name and dependencies,
  code in `src/<package>/`, tests in `tests/test_*.py`. `__init__.py`
  stays empty.
- **rust** - `Cargo.toml` with `[package]`, code from `src/lib.rs` or
  `src/main.rs`. A workspace root declaring no package is a container:
  its members are judged, it is not.
- **proto** - `buf.yaml` (v2) beside the `.proto` contracts. The contract
  is the source others generate from.

## The contract dance

1. `buf.yaml` and the contracts are the unit; lint and breaking run pinned.
2. `buf.gen.yaml` says what generation lands where; generated files
   (`*_pb2.py`, `*.pb.ts`) live in the consuming unit, scanned and never
   judged - their imports reach, their style is the generator's.
3. The drift stage holds generated code to its contract on every gate, and
   `gate --fix` regenerates after a contract edit.
4. `arbiter ledger sign .` records the contracts image; breaking is judged
   against it from then on. The user signs, never you.
5. A consumer imports the generated name and the graph resolves it to the
   contract, so a message nothing touches invites deletion at the
   contract's own line.

## What this skill never does

Restate the law. `arbiter rules` holds every rule's one line and
`arbiter rules <rule>` the whys; the arbiter-quality skill holds the fix
table and what only the user decides.
