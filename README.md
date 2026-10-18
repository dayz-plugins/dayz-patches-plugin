# dayz-patches-plugin

Workarounds for DayZ engine bugs, as a plugin for [uniloader](https://uniloaders.github.io/)
with the [DayZ adapter](https://github.com/dayz-plugins/dayz-adapter).

Nothing in here adds a feature. Every patch exists because the engine does something that
crashes or misbehaves, and every patch follows the same three rules:

- **it has a switch**, so a user who suspects it can turn it off without a new build,
- **it counts what it did**, visible in the console's `patches.status`,
- **it says why in the log**, the first few times and then periodically, never at frame rate.

A workaround nobody can switch off and nobody can count is indistinguishable from a new bug.

## The patches

### `guard_prepared_view` — the crash after the window is dragged

`render.execute_view` loads the render context's prepared-view pointer (`+0xA10` on
1.29.163709) and dereferences it a few instructions later without checking it:

```
render.execute_view+0x43   mov  r8, [r14+0xA10]      ; the prepared view
render.execute_view+0x65   movss xmm0, [r8+0x1A0]    ; no null check anywhere
```

The view setup stores that pointer and finalize clears it. After the game window has been
dragged — moved, resized, restored — the engine has been seen executing a view whose pointer
was already cleared, and faults inside its own code.

Both of the engine's call sites ignore what `execute_view` returns, which is what makes this
fixable from outside: the plugin detours the function, reads the pointer, and when it is null
returns without making the call. The frame loses that one view; the game carries on.

The read of the game's memory happens inside a structured exception handler, because the
condition this exists for is a render context being torn down on another thread, and a
workaround that faults while checking for a fault is no workaround.

## Using it

```
patches.guard_prepared_view        # the switch, in the console or the settings editor
patches.status                     # on/off, how many views were skipped, how many forwarded
```

Put `patches.dll` into the game's `plugins/` folder next to `dayz.dll`. The plugin asks the
adapter whether `render.execute_view` resolves for this build before it detours anything, so a
game build the address database does not cover leaves the plugin not started, with that name
in the log, rather than detouring something else.

## Building

```bash
scripts/gate.sh --deploy
```

Needs sibling checkouts of [uniloader](https://github.com/uniloaders/uniloader) (at
`../../uniloader`) and [dayz-adapter](https://github.com/dayz-plugins/dayz-adapter), and the
`build-box` container for the toolchain. `--host` runs the host steps only.

## Related repositories

| Repository | What it is |
| --- | --- |
| [uniloader](https://github.com/uniloaders/uniloader) | The loader and the plugin SDK |
| [dayz-adapter](https://github.com/dayz-plugins/dayz-adapter) | The `dayz` plugin: addresses, game events, chat |
| [dayz-data](https://github.com/dayz-plugins/dayz-data) | Addresses and struct offsets per game build |
| [dayz-debug-plugin](https://github.com/dayz-plugins/dayz-debug-plugin) | The console and loader state on a socket |
| [dayz-plugins.github.io](https://github.com/dayz-plugins/dayz-plugins.github.io) | Design notes and research |

## License

[Unlicense](LICENSE).
