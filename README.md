# Arbiter

A quality gate for repositories worked on by agents at scale. Its pillars are
correctness, integrity and economy; economy is the new one: public surface
nothing uses is a finding, and the gate invites its deletion.

Four commands stand over one resolved graph, across Python, TypeScript, C++
and Rust:

- `arbiter economy <folder>` reports every public symbol with no user
  outside its own unit.
- `arbiter integrity <folder>` reports the structure broken across units:
  units importing each other in a cycle, and a name re-exported, whether by
  an `__init__.py` import or a TypeScript `export ... from`.
- `arbiter blast <name> <folder>` prints each definition of a name and
  every place using that one, so a same-named symbol elsewhere stays out.
- `arbiter findings <folder> [file...]` answers every check's findings as
  data, one JSON line each, held to the files a caller names.
- `arbiter ledger <folder>` lists every suppression, `#[allow]` and `unsafe`
  in Rust, `noqa` and `pyright: ignore` in Python, `biome-ignore` and
  `@ts-expect-error` in TypeScript, against the reference
  `arbiter accept <folder>` records in `.arbiter/reference.json`, so each
  run names what was added since. An entry is a file and the directive as
  written, never a line number, which moves with every edit above it. A
  blanket form, a bare `noqa`, a `type: ignore`, an inner `#![allow]`, a
  `@ts-nocheck`, quiets more than one named rule: it is a finding, never
  counted and never accepted.
- `arbiter verify <folder>` runs each pack's declared tools over every unit
  holding its manifest, one line per check, failing output shown last lines
  first. A unit of a language whose pack declares no tools is named, never
  passed over in silence; a unit on a test path is a fixture and is not
  judged; a workspace manifest with members and no code of its own is a
  container, named while its members are judged. Nothing to run at all is a
  failure, not a pass.

  Three adapters exist, each tool pinned so every unit is held to one bar:
  Rust runs rustfmt, clippy denying every warning, and cargo test; Python
  runs ruff format and check, basedpyright, and pytest, through uv; and
  TypeScript runs biome and tsc. A pack carries the configuration its tools
  read, ruff's TOML and biome's JSON, written outside the judged repository
  at a place `{pack}` in a declared command resolves to, so the bar is the
  pack's, never a tool's default, and a run changes nothing in the
  repository.

  Before a unit's tools run, `wiring` holds its declared environment to the
  pack's bar: settings may add, never lower. TypeScript's resolved tsconfig
  must state each option `packs/typescript/bar.toml` names, environment
  options staying the unit's own, and a compiler default that happens to
  match is not a declaration. A Python manifest setting that ignores a lint
  rule, excludes a source file, lowers the type checker or trims the tests
  is named with the reason `packs/python/bar.toml` gives. A Cargo `[lints]`
  entry turned to allow is named: the bar denies every warning. A unit with
  no manifest for its language has declared no environment, and wiring says
  so.

A use is resolved, never matched by name: an import reaches the file its
module path names, a call reaches a header the caller includes. Three units
each defining `value` stay three symbols.

- A unit is a folder that declares itself: a manifest such as pyproject.toml,
  Cargo.toml, package.json or go.mod, or a BUILD file read as one more
  dialect. The repository itself answers for a file no folder claims. No
  build system is run or required.
- A root is where reachability starts, so its symbols are never findings: a
  binary target's entry file, the module a Python manifest's entry points or
  scripts name, and the src/main.rs beside a Cargo manifest.
- A test keeps nothing alive, wherever it lives: a symbol only tests reach
  is reported. A `*_test` target's srcs are tests; where no target claims a
  file, its name and folder say.
- A language is a pack: one module behind one trait, whose
  `packs/<language>/*.scm` tree-sitter queries say what defines a symbol,
  what uses one, and what a file mentions, and whose
  `packs/<language>/tools.toml` is its adapter: the tools holding a unit of
  that language to the bar. The core resolves, judges and runs; it knows no
  language.

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

Arbiter holds itself to its own bar: `arbiter verify .` runs the Rust
adapter, rustfmt, clippy denying every warning, and its tests, three checks
in under three seconds. The judgment is itself tested, against a committed
corpus with a known answer. A file formatted wrongly on purpose failed two
of the three checks with the diff in the output, and the exit code said so.
