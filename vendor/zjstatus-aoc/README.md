# zjstatus-aoc vendor snapshot

This directory contains the AOC-managed source snapshot for the custom Zellij top-bar plugin shipped as:

- `zellij/plugins/zjstatus-aoc.wasm`

## Upstream basis

- Project: `dj95/zjstatus`
- Upstream crate/package: `zjstatus`
- Seeded version: `0.22.0`

The vendored source is the canonical reviewed source for the managed AOC plugin build.

## AOC-specific changes

Compared with the upstream baseline, the AOC-managed variant includes patches for:

- reduced event subscriptions to avoid unnecessary mouse/pane/session churn
- `disable_mouse` actually disabling mouse handling/subscriptions
- grouped project-aware tabs with adjacent-project segment rendering
- theme updates via Zellij pipe messages (`aoc_theme`) without polling
- explicit tab project metadata via Zellij pipe messages (`aoc_tab_metadata`)
- runtime theme palette integration for the grouped tab renderer

## Build / verify

Rebuild the managed wasm into the repo:

```bash
bash scripts/zellij/rebuild-managed-plugin.sh
```

Verify the vendored source matches the committed wasm:

```bash
bash scripts/zellij/verify-managed-plugin.sh
```

Install/update the managed plugin into your Zellij config:

```bash
aoc-zellij-plugin install
```

## Notes

- The committed wasm is intentionally tracked so `install.sh` and `aoc-init` can repair/install the managed plugin without requiring a local Rust toolchain.
- CI verifies that the committed wasm stays in sync with this vendored source snapshot.
