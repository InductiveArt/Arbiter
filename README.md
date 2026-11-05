# Arbiter

One binary that judges a repository: the symbol graph resolved across its
languages, each unit's tools pinned to one bar, and the brief tying every
test to a need it answers. Agents work inside the judgment; the user signs
what it may not decide.

## Start

```
arbiter adopt .   # install the agent layer under .claude/
arbiter gate .    # judge; the first run seeds .arbiter/'s decision files
arbiter           # the verbs, one line each
```

## The shape

- A language is a pack - typescript, python, rust, cpp, proto, infra. The
  core resolves and judges one graph and knows no language.
- `arbiter rules` prints what holds where and each rule's one-line law;
  `arbiter rules <rule>` prints one rule's stance in every language, with
  each never's why.
- Decisions live in `.arbiter/`: `units.toml`, `licenses.toml`,
  `brief.toml`, and the reference accept records. The first gate seeds the
  toml files as comments; writing a real entry is the user's ruling, signed
  by committing. `last/` is the newest run's report and `warm/` the store;
  neither commits.
- `arbiter accept .` is the user's signature: suppressions, third parties
  and claims recorded. What leaves afterwards without a new signature is a
  finding.

## For agents

`adopt` installs the rules file, the arbiter-quality skill and a stop hook:
the entry protocol, each finding's expected fix, and what only the user
decides. The skill is arbiter-owned and refreshes with the binary; a test
holds it to each verb's own line, so the taught word cannot drift.
