# Arbiter

A quality gate for repositories worked on by agents at scale. Its pillars are
correctness, integrity and economy; economy is the new one: public surface
nothing uses is a finding, and the gate invites its deletion.

What exists is the economy spike, three commands over one resolved graph,
across Python, TypeScript, C++ and Rust:

- `arbiter economy <folder>` reports every public symbol with no user
  outside its own unit.
- `arbiter blast <name> <folder>` prints each definition of a name and
  every file using that one, so a same-named symbol elsewhere stays out.
- `arbiter findings <folder> [file...]` answers the same findings as data,
  one JSON line each, held to the files a caller names.

A use is resolved, never matched by name: an import reaches the file its
module path names, a call reaches a header the caller includes. Three units
each defining `value` stay three symbols.

- A unit is a folder with a BUILD file; the repository itself where none is.
- A root is where reachability starts, so its symbols are never findings: a
  binary target's entry file, the module a Python manifest's entry points or
  scripts name, and the src/main.rs beside a Cargo manifest.
- A test keeps nothing alive: a symbol only its own tests reach is reported.
  A `*_test` target's srcs are tests; where no target claims a file, its name
  and folder say.
- A language is a pack: one module behind one trait, whose
  `packs/<language>/*.scm` tree-sitter queries say what defines a symbol,
  what uses one, and what a file mentions. The core resolves and judges; it
  knows no language.

## Measured

Against a Bazel demo repository of the three languages, each unit defining a
`value`: the TypeScript one no one imports and the C++ one only its own test
calls are reported; the Python one an application imports is not, and the
application's `main` is exempt as the entry its BUILD target declares.
Removing that one import moves the Python `value` into the findings; giving
the TypeScript one a user in another unit removes it.

Against py-harness, 53 files and 391 public symbols in 0.3s: clean, after
one correction the run itself forced. The first run reported
`pytest_configure`, defined once and named nowhere, alive only through the
`pytest11` entry point its manifest declares; entry points now mark roots
the way a BUILD target's `main` does, and the run reports nothing.

Against a five-root monorepo whose every project covers its own `value` with
its own tests, 12 findings in 0.2s: each one a symbol no other project uses,
which per-project coverage can never see.

Rust arrived as the fourth language for the pack boundary's test: four core
lines changed, a module declaration, an import, a registry entry and a
dependency, and no logic anywhere. Arbiter judges its own source clean, all
15 public symbols used; a planted dead `pub fn` is reported at its line and
the run is clean again when it goes.
