# Agent notes: dayz-patches-plugin

## What this repository is

One plugin, `patches.dll`, holding workarounds for engine bugs. It is deliberately the least
clever repository in the org: each patch is a detour or a read, a switch, a counter and a log
line, and nothing else.

Prose documentation belongs in
[dayz-plugins.github.io](https://github.com/dayz-plugins/dayz-plugins.github.io); this
repository carries only `README.md` and `AGENTS.md`.

## Rules that matter

- **Every patch gets a setting, a counter and a log line.** No exceptions. A workaround that
  cannot be switched off cannot be ruled out when something else breaks, and one that does not
  count itself cannot be shown to be doing anything.
- **Never hard-code an address.** Ask the loader for a name from
  [dayz-data](https://github.com/dayz-plugins/dayz-data) and declare it with
  `require_symbols`, so a game update names a missing symbol in the log instead of detouring
  whatever now lives at that address.
- **A patch needs evidence before it ships.** The disassembly that shows the bug belongs in
  the README, and the symbol and offset it depends on belong in dayz-data with a note.
- **Nothing blocks in a detour.** The engine calls these from the render thread several times
  a frame: atomics only, never a lock the console or the UI could be holding.
- **Read the game's memory inside the structured handler.** These patches exist because the
  engine dereferences something it should not; checking for that must not be able to fault.
- **Only skip a call the engine ignores the result of.** `render.execute_view` is safe to skip
  because both of its call sites discard the return value — that was established by reading
  them, not assumed.

## Checks

```bash
scripts/build.sh            # fmt, clippy, tests, docs, then the Windows build
scripts/build.sh --check    # host gate only
scripts/build.sh --deploy   # also install patches.dll into the game directory
```

Zero warnings is the bar; the lints are set in the workspace manifest and `unsafe_code` is
denied except in the hook module, which says in a comment why it has to allow it.
