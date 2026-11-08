# Arbiter

One binary that judges a repository: the symbol graph resolved across its
languages, each unit's tools pinned to one bar, and the brief tying every
test to a need it answers. Agents work inside the judgment; the user signs
what it may not decide.

## Start

```
cargo install --git https://github.com/InductiveArt/Arbiter --tag v0.21.1 arbiter

arbiter adopt .   # install the agent layer under .claude/
arbiter gate .    # judge; the first run seeds .arbiter/'s decision files
arbiter           # the verbs, one line each
```

The acts are `gate`, `adopt` and `ledger sign`; the reads - `find`
(findings by rule, fallout of a name, deps of a unit, rhymes), `ledger`,
`brief`, `rules` - never write. Every finding names the law it breaks,
and the same word drills it: `arbiter rules <rule>` for the law,
`arbiter find <rule> .` for the instances.

The binary is self-contained; the machine still needs the toolchains the
repository's languages demand, and the gate names a missing one. Updating is
installing the next tag: the judge is pinned like everything it pins.

## The shape

- `processes.md` is the plan: the axes the work stands on, why each
  process step exists, and the constitution no step may cross.
- A language is a pack - typescript, python, rust, cpp, proto, infra. The
  core resolves and judges one graph and knows no language.
- `arbiter rules` prints what holds where and each rule's one-line law;
  `arbiter rules <rule>` prints one rule's stance in every language, with
  each never's why.
- Decisions live in `.arbiter/`: `units.toml`, `licenses.toml`,
  `brief.toml`, and the reference the signature records. The first gate seeds the
  toml files as comments; writing a real entry is the user's ruling, signed
  by committing. `last/` is the newest run's report and `warm/` the store;
  neither commits.
- `arbiter ledger sign .` is the user's signature: suppressions, third parties
  and claims recorded. What leaves afterwards without a new signature is a
  finding.

## For agents

`adopt` installs the rules file, the arbiter-quality skill and a stop hook:
the entry protocol, each finding's expected fix, and what only the user
decides. The skill is the agent layer's and refreshes with the binary, never
backwards: each adopted file's last line names the arbiter that wrote it,
and a stale binary is told to update instead of writing. A test holds the
skill to each verb's own line, so the taught word cannot drift.
