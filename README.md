# Arbiter

A quality gate for repositories worked on by agents at scale. Its pillars are
correctness, integrity and economy; economy is the new one: public surface
nothing uses is a finding, and the gate invites its deletion.

What exists is the economy spike. `arbiter <folder>` reports every public
symbol with no user outside its own unit, across Python, TypeScript and C++.
A use is resolved, never matched by name: an import reaches the file its
module path names, a call reaches a header the caller includes. Three units
each defining `value` stay three symbols.

- A unit is a folder with a BUILD file; the repository itself where none is.
- A binary target's entry file is a root: reachability starts there, so its
  symbols are never findings.
- A test keeps nothing alive: a symbol only its own tests reach is reported.
  A `*_test` target's srcs are tests; where no target claims a file, its name
  and folder say.
- A language is a pack: `packs/<language>/*.scm` holds the tree-sitter
  queries saying what defines a symbol, what uses one, and what a file
  mentions. The core resolves and judges; it knows no language.

## Measured

Against a Bazel demo repository of the three languages, each unit defining a
`value`: the TypeScript one no one imports and the C++ one only its own test
calls are reported; the Python one an application imports is not, and the
application's `main` is exempt as the entry its BUILD target declares.
Removing that one import moves the Python `value` into the findings; giving
the TypeScript one a user in another unit removes it.

Against py-harness, 53 files and 398 public symbols in 0.3s: one finding,
`pytest_configure`, defined once and named nowhere else. It is alive only
through the `pytest11` entry point its manifest declares, which is the
spike's measured gap: a manifest's entry points must mark roots the way a
BUILD target's `main` already does.

Against a five-root monorepo whose every project covers its own `value` with
its own tests, 12 findings in 0.2s: each one a symbol no other project uses,
which per-project coverage can never see.
