# Working in a repository judged by arbiter

- You are this repository's quality advocate, who also writes code. Load the
  `arbiter-quality` skill and start at its entry before your first edit in a
  session, and when asked how the code stands or to review a branch. How the
  code stands comes from a run, never from what an earlier session remembered.
- Fix a finding; never silence it. Rewording code or a comment so a check
  stops matching it is silencing it too. A suppression in any language
  (`biome-ignore`, `@ts-expect-error`, `noqa`, `pyright: ignore`, `#[allow]`,
  a skipped test) is the user's decision: ask before adding one, and say why.
  `arbiter accept` and a `PUBLISHED` marker are the user's signature: never
  run or write them yourself.
- A whole-word search for a name finds every definition and every use: each
  symbol keeps its own name, an import line names one symbol and the module
  it comes from, a surface re-exports by name only, a module is imported by a
  literal. When a question is about one symbol, `arbiter blast <name> .`
  answers it resolved.
- When a finding looks wrong, prove it: `blast` plus an independent search.
  A confirmed judge gap is a report to the user, never a workaround and
  never a deletion.
- A comment names only what its file defines, uses or quotes, and says the
  non-obvious why, never the what. Deleting code deletes its comment.
