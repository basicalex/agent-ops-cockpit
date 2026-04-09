# Managed Zellij Top Bar

AOC ships a managed `zjstatus`-based top bar plugin as:

- repo source: `vendor/zjstatus-aoc/`
- repo wasm: `zellij/plugins/zjstatus-aoc.wasm`
- installed wasm: `~/.config/zellij/plugins/zjstatus-aoc.wasm`

## Repair / install

```bash
aoc-zellij-plugin install
```

`aoc-init` also repairs this managed plugin automatically.

## Inspect status

```bash
aoc-zellij-plugin status
```

Useful fields include:

- `dest_wasm`
- `cache_wasm`
- `reference_wasm`
- `dest_matches_reference`
- the SHA256 lines for each copy

## Verify

```bash
aoc-zellij-plugin verify
```

This exits non-zero if the installed plugin is missing or diverges from the managed bundled copy.

## Rebuild the tracked wasm from vendored source

```bash
bash scripts/zellij/rebuild-managed-plugin.sh
```

## Verify the tracked wasm matches the vendored source

```bash
bash scripts/zellij/verify-managed-plugin.sh
```

CI also runs the managed plugin smoke/build path to keep the source snapshot and committed wasm in sync.

## Tab project metadata

AOC layouts still broadcast project metadata at pane startup:

```bash
aoc-tab-metadata sync
```

You can inspect it manually with:

```bash
aoc-tab-metadata status
```

This metadata is useful for project context, but grouped-tab behavior is now driven by the tab name itself rather than runtime metadata.

## Numeric tab grouping

Grouping is now explicit and rename-driven.

Use the normal Zellij rename flow (`Ctrl+t r`) and, if desired, start the name with a number:

- raw/internal tab name: `2 PI Agent`
- normal top-bar rendering: `PI Agent`
- rename-mode/top-bar rendering: `2 PI Agent`
- another tab in the same group: `2 Review`
- ungrouped tab: `Logs`

Tabs with the same leading numeric prefix group together. Tabs without a numeric prefix stay ungrouped.

Helpful commands:

```bash
aoc-tab-group status
aoc-tab-group set 2
aoc-tab-group rename "Review"
aoc-tab-group clear
```

`aoc-new-tab` and `aoc-launch` now create plain short tab names by default. Add a numeric prefix only when you want grouping.

## Layout hygiene

AOC now treats `aoc` as the single official managed general-purpose layout.

Legacy managed layout names such as these should be removed and not reused:

- `unstat`
- `minimal`
- `aoc-zjstatus-single`
- `aoc-zjstatus-test`
- `aoc.hybrid`

AOC prunes these stale managed layout files during install. Dedicated Mission Control still uses its own explicit layout path and launcher.
